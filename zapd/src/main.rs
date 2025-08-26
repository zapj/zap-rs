//! Run with
//!
//! ```not_rust
//! cargo run -p example-low-level-rustls
//! ```
//! 
//! 基本完成

use std::time::Duration;

use axum::{extract::{Path, Request}, response::{IntoResponse, Response}, routing::get, Extension, Router};
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener};
use tokio_rustls::{
    rustls::{pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer}, ServerConfig},
    TlsAcceptor,
};
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tower_http::timeout::TimeoutLayer;
use tower_service::Service;

mod config;
mod db;
mod routers;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    println!("{:?}",config::new());
    let tls_acceptor = create_tls_acceptor("conf/zap.crt","conf/zap.key");
    let bind = "0.0.0.0:2600";
    let tcp_listener = TcpListener::bind(bind).await.unwrap();
    info!("Zap server listening on {bind}.");
    
    let conn = db::prepare_database().await.unwrap();
    let app = Router::new()
        .merge(routers::api_auth_routers())
        .route("/hello/{id}", get(hello))
        .route("/", get(handler))
        .layer((
            TraceLayer::new_for_http(),
            TimeoutLayer::new(Duration::from_secs(10)),
            Extension(conn),
        ));
    
    loop {
        let (stream, client_addr) = tcp_listener.accept().await.unwrap();
        
        // 设置连接超时
        stream.set_linger(Some(Duration::from_secs(5))).ok();
        
        let mut buf = [0; 1];
        let n = stream.peek(&mut buf).await.unwrap();

        if n > 0 && buf[0] == 0x16 {
            // TLS连接处理
            let tls_acceptor = tls_acceptor.clone();
            let app = app.clone();
            
            tokio::spawn(async move {
                let Ok(stream) = tls_acceptor.accept(stream).await else {
                    error!("Error during TLS handshake from {}", client_addr);
                    return;
                };

                let stream = TokioIo::new(stream);
                let hyper_service = hyper::service::service_fn(move |request: Request<hyper::body::Incoming>| {
                    app.clone().call(request)
                });

                let ret = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                    .serve_connection_with_upgrades(stream, hyper_service)
                    .await;

                if let Err(err) = ret {
                    warn!("Error serving TLS connection from {}: {}", client_addr, err);
                }
            });
        } else {
            tokio::spawn(async move {
                
                // 快速处理HTTP请求，直接返回301重定向
                if let Err(e) = handle_plain_http(stream, client_addr).await {
                    warn!("Error handling plain HTTP from {}: {}", client_addr, e);
                }
            });
        }
    }
}

// 专门处理非TLS HTTP连接，直接返回301重定向
async fn handle_plain_http(mut stream: tokio::net::TcpStream, _client_addr: std::net::SocketAddr) -> anyhow::Result<()> {
    // 读取请求头（只读取必要的部分）
    let mut buffer = [0; 1024];
    let n = stream.read(&mut buffer).await?;
    
    if n == 0 {
        return Ok(());
    }
    
    // 解析Host头
    let request = String::from_utf8_lossy(&buffer[..n]);
    let host_header = request.lines()
        .find(|line| line.starts_with("Host:") || line.starts_with("host:"))
        .map(|line| line[5..].trim());
    
    // // 构建重定向URL
    let host = host_header.unwrap_or("");
    let redirect_url = format!("https://{}/", host);
    if host == "" {
        let resp = Response::builder().status(200).body("It's Works").unwrap();
        stream.write_all(resp.body().as_bytes()).await?;
    }else{
        // 直接返回301响应
        let response = format!(
            "HTTP/1.1 301 Moved Permanently\r\nLocation: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            redirect_url
        );
        stream.write_all(response.as_bytes()).await?;
    }
    
    stream.flush().await?;
    Ok(())
}

async fn handler() -> &'static str {
    "Hello, World!"
}

async fn hello(Path(id): Path<u32>) -> impl IntoResponse {
    format!("id {}", id)
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