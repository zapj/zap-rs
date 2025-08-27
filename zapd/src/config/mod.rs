use std::{fs::{File, OpenOptions}, io::Read, sync::{OnceLock, RwLock}};
use tracing::{error, info};
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
    pub cert_file : String,
    pub key_file : String,
}

#[derive(Debug,Deserialize,Serialize)]
pub struct JWTConfig {
    pub jwt_secure:String,
    pub jwt_expire: u32
}

pub fn new() -> ZapConfig {
    ZapConfig {
        server : ServerConfig {
            address : "0.0.0.0".to_string(),
            port : 2600,
            cert_file : "conf/zap.crt".to_string(),
            key_file : "conf/zap.key".to_string(),
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
      let mut default_conf = new();
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
                    default_conf.jwt.jwt_secure = cnf.jwt.jwt_secure;
                    default_conf.jwt.jwt_expire = cnf.jwt.jwt_expire;
                },
                Err(_)=> {
                    info!("zap.yaml 文件不存在");
                    let file = OpenOptions::new().write(true).create(true).open("conf/zap.yaml");
                    if let Ok(f) = file {
                        let _ = serde_yaml::to_writer(f, &default_conf);
                    }
                }
            }
          }
          Err(_) => {
                let file = OpenOptions::new().write(true).create(true).open("conf/zap.yaml");
                if let Ok(f) = file {
                    let _ = serde_yaml::to_writer(f, &default_conf);
                }
            }
      };
      RwLock::new(default_conf)
    })
}
