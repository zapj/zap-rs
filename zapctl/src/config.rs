//! zapctl 的配置读取：仅解析 zap.yaml 中的 `db.path`。
//!
//! 查找顺序与 `zapd/src/config/mod.rs` 保持一致：
//! 1. 环境变量 `ZAP_CONFIG`
//! 2. 生产默认 `/etc/zap/zap.yaml`
//! 3. 开发回退 `conf/zap.yaml`

use std::path::PathBuf;

use serde::Deserialize;

const DEFAULT_DB_PATH: &str = "data/zap.db";

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub db: DbSection,
}

#[derive(Debug, Deserialize)]
pub struct DbSection {
    #[serde(default = "default_db_path")]
    pub path: String,
}

fn default_db_path() -> String {
    DEFAULT_DB_PATH.to_string()
}

impl Default for DbSection {
    fn default() -> Self {
        Self {
            path: default_db_path(),
        }
    }
}

/// 解析配置文件路径（与 zapd 一致）。
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

/// 计算数据库路径。`override_path`（来自 `--db`）优先级最高。
pub fn db_path(override_path: Option<&str>) -> String {
    if let Some(p) = override_path
        && !p.is_empty()
    {
        return p.to_string();
    }

    let cfg_file = config_path();
    if let Ok(content) = std::fs::read_to_string(&cfg_file)
        && let Ok(cfg) = serde_yaml::from_str::<Config>(&content)
        && !cfg.db.path.is_empty()
    {
        return cfg.db.path;
    }

    DEFAULT_DB_PATH.to_string()
}
