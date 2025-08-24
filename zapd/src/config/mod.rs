use std::sync::{OnceLock, RwLock};

use serde::{Deserialize, Serialize};

#[derive(Debug,Deserialize,Serialize)]
pub struct ZapConfig {
    pub server : ServerConfig,
    pub jwt: JWTConfig,
}

#[derive(Debug,Deserialize,Serialize)]
pub struct ServerConfig {
    pub address: String,
    pub port : u16 , 
    pub ssl : bool,
    pub cert_file : String,
    pub key_file : String,
}

#[derive(Debug,Deserialize,Serialize)]
pub struct JWTConfig {
    pub jwt_secure:String,
    pub jwt_expire: u32
}

pub static GLOBAL_ZAP_CONFIG: OnceLock<RwLock<ZapConfig>> = OnceLock::new();

pub fn new() -> ZapConfig {
    ZapConfig {
        server : ServerConfig {
            address : "0.0.0.0".to_string(),
            port : 2600,
            ssl: false,
            cert_file : "".to_string(),
            key_file : "".to_string(),
        },
        jwt : JWTConfig {
            jwt_secure : "secure-key-zap-default".to_string(),
            jwt_expire : 3600,
        }
    }
}

pub fn get_config() -> &'static RwLock<ZapConfig> {
    static GLOBAL_ZAP_CONFIG: OnceLock<RwLock<ZapConfig>> = OnceLock::new();
    GLOBAL_ZAP_CONFIG.get_or_init(|| {
      let default_conf = new();
      RwLock::new(default_conf)
    })
}
