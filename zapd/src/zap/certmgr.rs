use std::path::Path;
use std::process::Command;

use tracing::{info, warn};

/// Check if certificate files exist, generate self-signed certs if missing.
/// Returns true if certificates are ready (either existed or were generated).
pub fn ensure_certs(cert_file: &str, key_file: &str) -> bool {
    if Path::new(cert_file).exists() && Path::new(key_file).exists() {
        return true;
    }

    info!("TLS certificate files not found, generating self-signed certificate...");

    // Try using openssl command first
    if try_openssl_gen(cert_file, key_file) {
        info!("Self-signed certificate generated successfully using openssl");
        return true;
    }

    // Fallback: try using rcgen crate
    if try_rcgen_gen(cert_file, key_file) {
        info!("Self-signed certificate generated successfully using rcgen");
        return true;
    }

    warn!("Failed to generate self-signed certificate. HTTPS will not work properly.");
    false
}

fn try_openssl_gen(cert_file: &str, key_file: &str) -> bool {
    let output = Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:4096",
            "-keyout",
            key_file,
            "-out",
            cert_file,
            "-days",
            "3650",
            "-nodes",
            "-subj",
            "/CN=zap-local",
            "-addext",
            "subjectAltName=DNS:localhost,IP:127.0.0.1",
        ])
        .output();

    match output {
        Ok(o) if o.status.success() => true,
        Ok(o) => {
            warn!(
                "openssl failed: {}",
                String::from_utf8_lossy(&o.stderr)
            );
            false
        }
        Err(e) => {
            warn!("openssl not available: {}", e);
            false
        }
    }
}

fn try_rcgen_gen(cert_file: &str, key_file: &str) -> bool {
    // Use rcgen to generate a self-signed certificate
    let mut params = rcgen::CertificateParams::default();
    params
        .subject_alt_names
        .push(rcgen::SanType::DnsName("localhost".try_into().unwrap()));
    params
        .subject_alt_names
        .push(rcgen::SanType::IpAddress(std::net::IpAddr::V4(
            std::net::Ipv4Addr::new(127, 0, 0, 1),
        )));

    let key_pair = match rcgen::KeyPair::generate() {
        Ok(kp) => kp,
        Err(e) => {
            warn!("rcgen key generation failed: {}", e);
            return false;
        }
    };

    let cert = match params.self_signed(&key_pair) {
        Ok(c) => c,
        Err(e) => {
            warn!("rcgen cert generation failed: {}", e);
            return false;
        }
    };

    // Write private key
    match std::fs::write(key_file, key_pair.serialize_pem()) {
        Ok(_) => {}
        Err(e) => {
            warn!("failed to write key file: {}", e);
            return false;
        }
    }

    // Write certificate
    match std::fs::write(cert_file, cert.pem()) {
        Ok(_) => {}
        Err(e) => {
            warn!("failed to write cert file: {}", e);
            return false;
        }
    }

    true
}
