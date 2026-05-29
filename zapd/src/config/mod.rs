use std::{
    fs::OpenOptions,
    io::Read,
    sync::{OnceLock, RwLock},
};

use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Deserialize, Serialize)]
pub struct ZapConfig {
    pub server: ServerConfig,
    pub jwt: JWTConfig,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ServerConfig {
    pub address: String,
    pub port: u16,
    pub cert_file: String,
    pub key_file: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct JWTConfig {
    pub jwt_secure: String,
    pub jwt_expire: u64,
}

const DEFAULT_JWT_SECURE: &str = "secure-key-zap-default";

pub fn new() -> ZapConfig {
    ZapConfig {
        server: ServerConfig {
            address: "0.0.0.0".to_string(),
            port: 2600,
            cert_file: "conf/zap.crt".to_string(),
            key_file: "conf/zap.key".to_string(),
        },
        jwt: JWTConfig {
            jwt_secure: DEFAULT_JWT_SECURE.to_string(),
            jwt_expire: 3600,
        },
    }
}

/// Generate a random hex string of given byte length
fn generate_random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::getrandom(&mut buf).expect("failed to generate random bytes");
    hex::encode(buf)
}

/// Check if JWT secret needs rotation (still using default)
fn needs_jwt_rotation(secret: &str) -> bool {
    secret == DEFAULT_JWT_SECURE || secret.is_empty()
}

fn write_config_to_file(config: &ZapConfig) {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open("conf/zap.yaml");
    match file {
        Ok(f) => {
            if let Err(e) = serde_yaml::to_writer(f, config) {
                info!("failed to write config: {}", e);
            }
        }
        Err(e) => {
            info!("failed to open config file for writing: {}", e);
        }
    }
}

pub fn get_config() -> &'static RwLock<ZapConfig> {
    static GLOBAL_ZAP_CONFIG: OnceLock<RwLock<ZapConfig>> = OnceLock::new();
    GLOBAL_ZAP_CONFIG.get_or_init(|| {
        let mut default_conf = new();
        let mut needs_write = false;

        let file = OpenOptions::new().read(true).write(true).open("conf/zap.yaml");
        match file {
            Ok(mut f) => {
                let mut buffer = String::new();
                let _ = f.read_to_string(&mut buffer);
                match serde_yaml::from_str::<ZapConfig>(&buffer) {
                    Ok(cnf) => {
                        default_conf.server.address = cnf.server.address;
                        default_conf.server.port = cnf.server.port;
                        default_conf.server.cert_file = cnf.server.cert_file;
                        default_conf.server.key_file = cnf.server.key_file;

                        // Rotate JWT secret if still using default
                        if needs_jwt_rotation(&cnf.jwt.jwt_secure) {
                            let new_secret = generate_random_hex(32);
                            info!("JWT secret was using default value, generated new random key");
                            default_conf.jwt.jwt_secure = new_secret;
                            needs_write = true;
                        } else {
                            default_conf.jwt.jwt_secure = cnf.jwt.jwt_secure;
                        }
                        default_conf.jwt.jwt_expire = cnf.jwt.jwt_expire;
                    }
                    Err(_) => {
                        info!("failed to parse zap.yaml, using defaults");
                        // Generate random JWT secret for fresh config
                        default_conf.jwt.jwt_secure = generate_random_hex(32);
                        needs_write = true;
                    }
                }
            }
            Err(_) => {
                info!("zap.yaml not found, creating with generated secrets");
                // Generate random JWT secret for new installation
                default_conf.jwt.jwt_secure = generate_random_hex(32);
                needs_write = true;
            }
        };

        // Persist updated config if changes were made
        if needs_write {
            write_config_to_file(&default_conf);
        }

        RwLock::new(default_conf)
    })
}
