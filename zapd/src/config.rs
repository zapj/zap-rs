use serde::{Deserialize, Serialize};


#[derive(Debug,Deserialize,Serialize)]
pub struct ZapConfig {
    server : ServerConfig,
    jwt: JWTConfig,
}

#[derive(Debug,Deserialize,Serialize)]
pub struct ServerConfig {
    address: String,
    port : u16 , 
    ssl : bool,
    cert_file : String,
    key_file : String,
}

#[derive(Debug,Deserialize,Serialize)]
pub struct JWTConfig {
    jwt_secure:String,
    jwt_expire: u32
}


pub fn new() -> ZapConfig {
    ZapConfig {
        server : ServerConfig {
            address : "127.0.0.1".to_string(),
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