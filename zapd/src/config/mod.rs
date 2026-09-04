use std::{
    fs::OpenOptions,
    io::Read,
    path::PathBuf,
    sync::{OnceLock, RwLock},
};

use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Deserialize, Serialize)]
pub struct ZapConfig {
    pub server: ServerConfig,
    pub jwt: JWTConfig,
    #[serde(default)]
    pub exec: ExecConfig,
    #[serde(default)]
    pub db: DbConfig,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ServerConfig {
    pub address: String,
    pub port: u16,
    pub cert_file: String,
    pub key_file: String,
    /// 统一 URL 前缀。配置如 `zap` 后，页面与接口全部位于 `/zap/` 下
    /// （例：`https://host:2600/zap/dashboard`、`/zap/api/auth/login`）。
    /// 留空则不启用前缀，行为与之前完全一致。
    #[serde(default)]
    pub url_prefix: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct JWTConfig {
    pub jwt_secure: String,
    pub jwt_expire: u64,
}

/// `zapd` 与 `zapexec` 之间的 IPC 配置。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExecConfig {
    pub socket_path: String,
    pub secret_path: String,
}

impl Default for ExecConfig {
    fn default() -> Self {
        Self {
            socket_path: "/run/zap/exec.sock".to_string(),
            secret_path: "/etc/zap/exec.key".to_string(),
        }
    }
}

/// SQLite 数据库配置（`zapd` 与 `zapctl` 共用）。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DbConfig {
    pub path: String,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            path: "data/zap.db".to_string(),
        }
    }
}

const DEFAULT_JWT_SECURE: &str = "secure-key-zap-default";

/// 规范化 URL 前缀：去掉首尾空白与斜杠，并过滤掉空白段。
///
/// `"/zap/"` → `"zap"`，`"zap"` → `"zap"`，`""` → `""`（不启用前缀）。
/// 拼接时再补上 `/`，即 `/{prefix}`。
pub fn normalize_url_prefix(raw: &str) -> String {
    raw.trim()
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim())
        .collect::<Vec<_>>()
        .join("/")
}

/// 读取规范化后的 URL 前缀（不带首尾斜杠）。空串表示未启用前缀。
pub fn url_prefix() -> String {
    get_config()
        .read()
        .map(|c| c.server.url_prefix.clone())
        .unwrap_or_default()
}

/// 拼好的前缀路径：`/zap`；未启用时为空串（便于直接拼接路径）。
pub fn url_prefix_path() -> String {
    let p = url_prefix();
    if p.is_empty() {
        String::new()
    } else {
        format!("/{p}")
    }
}

pub fn new() -> ZapConfig {
    ZapConfig {
        server: ServerConfig {
            address: "0.0.0.0".to_string(),
            port: 2600,
            cert_file: "conf/zap.crt".to_string(),
            key_file: "conf/zap.key".to_string(),
            url_prefix: String::new(),
        },
        jwt: JWTConfig {
            jwt_secure: DEFAULT_JWT_SECURE.to_string(),
            jwt_expire: 3600,
        },
        exec: ExecConfig::default(),
        db: DbConfig::default(),
    }
}

/// 配置文件路径（启动时打印，便于排查"改了配置没生效"）：
/// 1. 环境变量 `ZAP_CONFIG`（若设置）
/// 2. 生产默认 `/etc/zap/zap.yaml`（若存在）——注意它优先于 `conf/zap.yaml`
/// 3. 开发回退 `conf/zap.yaml`
pub fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("ZAP_CONFIG")
        && !p.is_empty()
    {
        return PathBuf::from(p);
    }
    let prod = PathBuf::from("/etc/zap/zap.yaml");
    if prod.exists() {
        return prod;
    }
    PathBuf::from("conf/zap.yaml")
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
        .open(config_path());
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

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(config_path());
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
                        default_conf.server.url_prefix = normalize_url_prefix(&cnf.server.url_prefix);
                        default_conf.exec = cnf.exec;

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
                        default_conf.db = cnf.db;
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
