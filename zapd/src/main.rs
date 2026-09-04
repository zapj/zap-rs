use std::{env, time::Duration};

use axum::{Router, extract::Request};
use clap::Parser;
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::net::TcpListener;
use tokio_rustls::{
    TlsAcceptor,
    rustls::{
        ServerConfig,
        pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
    },
};
use tower_http::compression::CompressionLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tower_service::Service;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod db;
mod routers;
pub mod zap;
pub mod zapexec;

#[derive(clap::Parser)]
struct Cli {
    #[clap(short, long, action)]
    version: bool,
}

#[tokio::main]
async fn main() {
    // Install rustls crypto provider before any TLS operations
    // (required by rustls 0.23+ when used through rcgen + tokio-rustls)
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let cli = Cli::parse();
    if cli.version {
        println!("zapd version {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!(
                    "{}=debug,tower_http=debug,axum::rejection=trace",
                    env!("CARGO_CRATE_NAME")
                )
                .into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 打印实际生效的配置文件：生产环境 /etc/zap/zap.yaml 优先于 conf/zap.yaml，
    // rundev.sh 则通过 ZAP_CONFIG 指向 data/run/zap.dev.yaml。
    // 显示绝对路径 + 存在性，便于排查"改了配置但没生效"。
    let cfg_path = config::config_path();
    let cfg_display = cfg_path.canonicalize().unwrap_or_else(|_| cfg_path.clone());
    if cfg_path.exists() {
        info!("using config file: {}", cfg_display.display());
    } else {
        warn!(
            "配置文件不存在，将使用内置默认值（url_prefix 等设置不会生效）: {}",
            cfg_display.display()
        );
    }

    // 一次性读取配置并转为 owned 值：配置读写锁不跨 await 持有
    let (cert_file, key_file, bind, web_port) = {
        let cfg = config::get_config().read().unwrap();
        (
            cfg.server.cert_file.clone(),
            cfg.server.key_file.clone(),
            format!("{}:{}", cfg.server.address, cfg.server.port),
            cfg.server.port,
        )
    };
    // 统一 URL 前缀（server.url_prefix）：留空则不启用
    let url_prefix = config::url_prefix();

    // Ensure TLS certificates exist (generate self-signed if missing)
    if !zap::certmgr::ensure_certs(&cert_file, &key_file) {
        warn!(
            "TLS certificates not available at {} / {}. HTTPS will not work.",
            cert_file, key_file
        );
    }

    let tls_acceptor = create_tls_acceptor(&cert_file, &key_file);
    let tcp_listener = TcpListener::bind(&bind).await.unwrap();
    let primary_ip = local_ip_address::local_ip()
        .unwrap_or_else(|_| std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)));
    info!("listening on https://{}:{}", primary_ip, web_port);
    info!("Zap server listening on https://{}.", bind);
    if url_prefix.is_empty() {
        info!("URL prefix: (none) — 页面在 / ，接口在 /api/");
    } else {
        info!(
            "URL prefix: /{} — 页面在 /{}/ ，接口在 /{}/api/",
            url_prefix, url_prefix, url_prefix
        );
    }

    // init db
    db::init_db::init_schema().await;

    // Security: admin password hint (only relevant when a fresh DB was created)
    match std::env::var("ZAP_ADMIN_PASSWORD").map(|p| p.trim().to_string()) {
        Ok(p) if !p.is_empty() => {
            info!("Admin password initialized from ZAP_ADMIN_PASSWORD on fresh DB.");
        }
        _ => {
            warn!(
                "Default admin password is '123456'. Please change it immediately after first login."
            );
        }
    }

    // init job scheduler for system monitoring
    zap::job::init_system_jobs().await;
    // init cron scheduler for 脚本/自动化 计划任务
    zap::script_cron::start();
    // 自动更新（zapd/zapexec 系统升级）定时调度
    zap::auto_update::start();

    let app = Router::new().merge(routers::routers()).layer((
        TraceLayer::new_for_http(),
        TimeoutLayer::new(Duration::from_secs(10)),
        CompressionLayer::new(),
    ));

    loop {
        let (stream, client_addr) = match tcp_listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                error!("failed to accept connection: {}", e);
                continue;
            }
        };

        stream.set_linger(Some(Duration::from_secs(30))).ok();

        // A TLS ClientHello always starts with byte 0x16.
        // Peek one byte to tell HTTPS from plain HTTP on the same port.
        let mut buf = [0; 1];
        let n = match stream.peek(&mut buf).await {
            Ok(n) => n,
            Err(_) => continue,
        };

        if n > 0 && buf[0] == 0x16 {
            let tls_acceptor = tls_acceptor.clone();
            let app = app.clone();
            tokio::spawn(async move {
                serve_tls_connection(stream, client_addr, tls_acceptor, app).await;
            });
        } else {
            tokio::spawn(async move {
                if let Err(e) = serve_plain_http(stream, client_addr).await {
                    warn!("Error serving plain HTTP from {}: {}", client_addr, e);
                }
            });
        }
    }
}

/// Serve a TLS connection with the axum app (HTTP/1.1 + HTTP/2 via ALPN).
async fn serve_tls_connection(
    stream: tokio::net::TcpStream,
    client_addr: std::net::SocketAddr,
    tls_acceptor: TlsAcceptor,
    app: Router,
) {
    let stream = match tls_acceptor.accept(stream).await {
        Ok(stream) => stream,
        Err(_) => {
            error!("Error during TLS handshake from {}", client_addr);
            return;
        }
    };

    let stream = TokioIo::new(stream);
    let hyper_service =
        hyper::service::service_fn(move |mut request: Request<hyper::body::Incoming>| {
            request.extensions_mut().insert(client_addr);
            app.clone().call(request)
        });

    if let Err(err) = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
        .serve_connection_with_upgrades(stream, hyper_service)
        .await
    {
        warn!("Error serving TLS connection from {}: {}", client_addr, err);
    }
}

/// Serve a plain HTTP connection: redirect every request to HTTPS (301),
/// preserving the host, path and query string. Uses hyper's standard HTTP/1.1
/// parser instead of manual parsing, so oversized headers and odd requests
/// are handled correctly.
async fn serve_plain_http(
    stream: tokio::net::TcpStream,
    _client_addr: std::net::SocketAddr,
) -> anyhow::Result<()> {
    let redirect =
        hyper::service::service_fn(move |req: Request<hyper::body::Incoming>| async move {
            let host = req
                .headers()
                .get(hyper::header::HOST)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
                .unwrap_or_default();
            let target = req
                .uri()
                .path_and_query()
                .map(|p| p.as_str())
                .unwrap_or("/");

            let response = if host.is_empty() {
                // HTTP/1.1 requires a Host header — reject the request otherwise.
                axum::http::Response::builder()
                    .status(400)
                    .header("Content-Length", "0")
                    .body(axum::body::Body::empty())
                    .unwrap()
            } else {
                axum::http::Response::builder()
                    .status(301)
                    .header("Location", format!("https://{}{}", host, target))
                    .header("Content-Length", "0")
                    .header("Connection", "close")
                    .body(axum::body::Body::empty())
                    .unwrap()
            };
            Ok::<_, std::convert::Infallible>(response)
        });

    hyper::server::conn::http1::Builder::new()
        .serve_connection(TokioIo::new(stream), redirect)
        .await?;

    Ok(())
}

fn create_tls_acceptor(cert: &str, key: &str) -> TlsAcceptor {
    let key = PrivateKeyDer::from_pem_file(key).unwrap();
    let certs = CertificateDer::pem_file_iter(cert)
        .unwrap()
        .map(|cert| cert.unwrap())
        .collect();

    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .expect("bad certificate/key");

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    TlsAcceptor::from(std::sync::Arc::new(config))
}
