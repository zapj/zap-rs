
use std::time::Duration;
use std::sync::{RwLock,OnceLock};
use axum::{extract::{Path, Request}, response::IntoResponse, routing::get, Extension, Router};
use config::ZapConfig;
use hyper::body::Incoming;
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::io::AsyncWriteExt;
use tokio_rustls::{
    rustls::{pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer}, ServerConfig},
    TlsAcceptor,
};
use tower_http::trace::TraceLayer;
use tower_service::Service;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tokio::net::TcpListener;
use tower_http::timeout::TimeoutLayer;

mod config;
mod db;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    
    println!("{:?}",config::get_config().read().unwrap());
    let pub_id_addr = match public_ip_address::perform_lookup(None).await {
        Ok(ip) => {
            ip.ip.to_string()
        }
        Err(_) => {
            "127.0.0.1".to_string()
        }
    };
    // let mut cnf  = config::get_config().write().unwrap();
    // cnf.server.address = pub_id_addr;

    // println!("{:?}", *cnf);
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    
    let tls_acceptor = create_tls_acceptor("conf/zap.crt","conf/zap.key");
    let bind = "0.0.0.0:2600";
    let tcp_listener = TcpListener::bind(bind).await.unwrap();
    info!("Zap server listening on {bind}. ");
    let conn = db::prepare_database().await.unwrap();
    let app = Router::new().route("/hello/{id}", get(hello)).route("/", get(handler)).layer((
        TraceLayer::new_for_http(),
        // Graceful shutdown will wait for outstanding requests to complete. Add a timeout so
        // requests don't hang forever.
        TimeoutLayer::new(Duration::from_secs(10)),
        Extension(conn),
    ));
    
    loop {
        let tower_service = app.clone();
        

        // Wait for new tcp connection
        let (mut stream, addr) = tcp_listener.accept().await.unwrap();
        let mut buf = [0; 1];
        let n = stream.peek(&mut buf).await.unwrap();

        if n > 0 && buf[0] == 0x16 {
        
            let tls_acceptor = tls_acceptor.clone();
            tokio::spawn(async move {
            // Wait for tls handshake to happen
            let Ok(stream) = tls_acceptor.accept(stream).await else {
                error!("error during tls handshake connection from {}", addr);
                return;
            };

            // Hyper has its own `AsyncRead` and `AsyncWrite` traits and doesn't use tokio.
            // `TokioIo` converts between them.
            let stream = TokioIo::new(stream);

            // Hyper also has its own `Service` trait and doesn't use tower. We can use
            // `hyper::service::service_fn` to create a hyper `Service` that calls our app through
            // `tower::Service::call`.
            let hyper_service = hyper::service::service_fn(move |request: Request<Incoming>| {
                // We have to clone `tower_service` because hyper's `Service` uses `&self` whereas
                // tower's `Service` requires `&mut self`.
                //
                // We don't need to call `poll_ready` since `Router` is always ready.
                tower_service.clone().call(request)
            });

            let ret = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(stream, hyper_service)
                .await;

            if let Err(err) = ret {
                warn!("error serving connection from {}: {}", addr, err);
            }
            });
        }else {
            info!("request addr {}",addr);
            let resp = format!(
                "HTTP/1.1 301 Moved Permanently\r\nLocation: https://{host}:{port}/\r\nContent-Length: 0\r\n\r\n",
                host = addr.ip(),
                port = 2600
            );
            let _ = stream.write_all(resp.as_bytes()).await;
            let _ = stream.flush().await;
            continue;
        }
    
    }
}

async fn handler() -> &'static str {
    "Hello, World!"
}

async fn hello(Path(id):Path<u32>) -> impl IntoResponse {
    format!("id {}",id)
}


fn create_tls_acceptor(cert: &str,key :&str) -> TlsAcceptor {
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

