//! `zapctl env` 子命令：管理 `server_env` 表（运行环境状态表）键值。
//!
//! 表中数据按 scope 分为两类：
//! - `conf`：管理员维护的全局配置（webserver / php_default / database /
//!   vhost_mode / fpm_pool_defaults / user_home_root / basic_* 等）；
//! - `auto`：zapexec 自动探测的快照（k='payload'），由面板自动刷新，**只读**。
//!
//! 本子命令直连 SQLite（与 `zapctl user` 一致），写操作需 root。
//! 仅对少数「写错即功能异常」的键做轻量校验，其余交给面板逻辑兜底。

use std::io::Read;
use std::time::Duration;

use clap::{Subcommand, ValueEnum};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use crate::{NC, YELLOW, ensure_root, ok};

/// 键名最大长度。
const MAX_KEY_LEN: usize = 128;
/// 值最大长度（64 KiB）。
const MAX_VALUE_LEN: usize = 64 * 1024;
/// 打开数据库时的忙等待上限，降低与 zapd 并发写冲突。
const BUSY_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Scope {
    /// 管理员全局配置
    Conf,
    /// 自动探测快照（只读）
    Auto,
}

impl Scope {
    fn as_str(self) -> &'static str {
        match self {
            Scope::Conf => "conf",
            Scope::Auto => "auto",
        }
    }
}

#[derive(Subcommand)]
pub enum EnvCommand {
    /// 列出记录（缺省列出全部 scope）
    List {
        /// 仅列出指定 scope
        #[arg(long, value_enum)]
        scope: Option<Scope>,
        /// 以 JSON 输出
        #[arg(long)]
        json: bool,
    },
    /// 读取单个键
    Get {
        /// 键名
        key: String,
        /// 目标 scope
        #[arg(long, value_enum, default_value_t = Scope::Conf)]
        scope: Scope,
        /// 以 JSON 输出
        #[arg(long)]
        json: bool,
    },
    /// 新增或修改键值（upsert）
    Set {
        /// 键名
        key: String,
        /// 值；传 `-` 表示从标准输入读取
        value: String,
        /// 目标 scope
        #[arg(long, value_enum, default_value_t = Scope::Conf)]
        scope: Scope,
        /// 备注（缺省保留原备注）
        #[arg(long)]
        remark: Option<String>,
        /// 以 JSON 输出
        #[arg(long)]
        json: bool,
    },
    /// 删除键
    #[command(alias = "rm", alias = "delete")]
    Unset {
        /// 键名
        key: String,
        /// 目标 scope
        #[arg(long, value_enum, default_value_t = Scope::Conf)]
        scope: Scope,
        /// 以 JSON 输出
        #[arg(long)]
        json: bool,
    },
    /// 从文件批量导入（逐行 `key=value`，忽略空行与 # 注释）
    Import {
        /// 文件路径（`-` 表示标准输入）
        file: String,
        /// 目标 scope
        #[arg(long, value_enum, default_value_t = Scope::Conf)]
        scope: Scope,
        /// 仅校验并预览，不写入
        #[arg(long)]
        dry_run: bool,
        /// 跳过非法行（默认遇错中止）
        #[arg(long)]
        skip_errors: bool,
        /// 以 JSON 输出
        #[arg(long)]
        json: bool,
    },
}

pub fn dispatch(cmd: EnvCommand, db_path: &str) -> Result<(), String> {
    match cmd {
        EnvCommand::List { scope, json } => cmd_list(db_path, scope, json),
        EnvCommand::Get { key, scope, json } => cmd_get(db_path, &key, scope, json),
        EnvCommand::Set {
            key,
            value,
            scope,
            remark,
            json,
        } => cmd_set(db_path, &key, &value, scope, remark.as_deref(), json),
        EnvCommand::Unset { key, scope, json } => cmd_unset(db_path, &key, scope, json),
        EnvCommand::Import {
            file,
            scope,
            dry_run,
            skip_errors,
            json,
        } => cmd_import(db_path, &file, scope, dry_run, skip_errors, json),
    }
}

// ── 数据库 ────────────────────────────────────────────────────

fn open(db_path: &str) -> Result<Connection, String> {
    let conn = Connection::open(db_path).map_err(|e| format!("无法打开数据库 {db_path}: {e}"))?;
    // 与 zapd 并发访问时避免直接报 database is locked
    let _ = conn.busy_timeout(BUSY_TIMEOUT);
    Ok(conn)
}

/// 写入单条记录：存在则更新，不存在则新增。
/// `remark` 为 `None` 时保留原有备注（新增时写入空串）。
fn upsert(
    conn: &Connection,
    scope: Scope,
    key: &str,
    value: &str,
    remark: Option<&str>,
) -> Result<(), String> {
    let existed: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM server_env WHERE scope = ?1 AND k = ?2)",
            params![scope.as_str(), key],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    let now = chrono::Local::now().timestamp();
    if existed {
        match remark {
            Some(r) => conn.execute(
                "UPDATE server_env SET v = ?1, remark = ?2, updated_at = ?3
                 WHERE scope = ?4 AND k = ?5",
                params![value, r, now, scope.as_str(), key],
            ),
            None => conn.execute(
                "UPDATE server_env SET v = ?1, updated_at = ?2 WHERE scope = ?3 AND k = ?4",
                params![value, now, scope.as_str(), key],
            ),
        }
    } else {
        conn.execute(
            "INSERT INTO server_env (scope, k, v, remark, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![scope.as_str(), key, value, remark.unwrap_or(""), now],
        )
    }
    .map_err(|e| format!("写入失败: {e}"))?;
    Ok(())
}

// ── 校验 ──────────────────────────────────────────────────────

fn validate_key(key: &str) -> Result<(), String> {
    if key.is_empty() {
        return Err("键名不能为空".to_string());
    }
    if key.len() > MAX_KEY_LEN {
        return Err(format!("键名长度超限（最大 {MAX_KEY_LEN} 字符）"));
    }
    if !key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    {
        return Err("键名仅支持字母 / 数字 / 下划线 / 点 / 短横线".to_string());
    }
    Ok(())
}

fn validate_value(value: &str) -> Result<(), String> {
    if value.len() > MAX_VALUE_LEN {
        return Err(format!("值长度超限（最大 {MAX_VALUE_LEN} 字节）"));
    }
    Ok(())
}

/// 对少数「写错即功能异常」的键做轻量校验（与面板写入规则保持一致）。
fn validate_known_key(key: &str, value: &str) -> Result<(), String> {
    match key {
        "vhost_mode" if !["www", "system"].contains(&value) => {
            Err("vhost_mode 仅支持 www / system".to_string())
        }
        "fpm_pool_defaults"
            if !value.is_empty()
                && !matches!(serde_json::from_str::<Value>(value), Ok(Value::Object(_))) =>
        {
            Err("fpm_pool_defaults 必须是 JSON 对象".to_string())
        }
        "basic_mail_encryption" if !["ssl", "tls", "none"].contains(&value) => {
            Err("basic_mail_encryption 仅支持 ssl / tls / none".to_string())
        }
        _ => Ok(()),
    }
}

/// 写操作前的公共检查：禁止写 auto 快照。
fn ensure_writable(scope: Scope) -> Result<(), String> {
    if scope == Scope::Auto {
        return Err("scope=auto 为自动探测快照（只读），不允许写入".to_string());
    }
    Ok(())
}

// ── 子命令实现 ────────────────────────────────────────────────

fn cmd_list(db_path: &str, scope: Option<Scope>, json_out: bool) -> Result<(), String> {
    let conn = open(db_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, scope, k, v, remark, updated_at FROM server_env
             WHERE (?1 IS NULL OR scope = ?1) ORDER BY scope, k",
        )
        .map_err(|e| e.to_string())?;

    let rows: Vec<(i64, String, String, String, String, i64)> = stmt
        .query_map(params![scope.map(Scope::as_str)], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    if json_out {
        let arr: Vec<Value> = rows
            .iter()
            .map(|(id, scope, k, v, remark, ts)| {
                json!({
                    "id": id,
                    "scope": scope,
                    "key": k,
                    "value": v,
                    "remark": remark,
                    "updated_at": ts,
                })
            })
            .collect();
        println!("{}", to_json(&Value::Array(arr))?);
        return Ok(());
    }

    if rows.is_empty() {
        println!("(无记录)");
        return Ok(());
    }

    println!(
        "{:<6} {:<6} {:<28} {:<40} {:<16} {:<20}",
        "ID", "SCOPE", "KEY", "VALUE", "REMARK", "UPDATED"
    );
    println!("{}", "-".repeat(122));
    for (id, scope, k, v, remark, ts) in &rows {
        println!(
            "{id:<6} {scope:<6} {:<28} {:<40} {:<16} {:<20}",
            ellipsis(k, 28),
            ellipsis(v, 40),
            ellipsis(remark, 16),
            format_time(*ts),
        );
    }
    Ok(())
}

fn cmd_get(db_path: &str, key: &str, scope: Scope, json_out: bool) -> Result<(), String> {
    validate_key(key)?;
    let conn = open(db_path)?;
    let row: Option<(String, String, i64)> = conn
        .query_row(
            "SELECT v, remark, updated_at FROM server_env WHERE scope = ?1 AND k = ?2",
            params![scope.as_str(), key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    let Some((value, remark, ts)) = row else {
        return Err(format!("{}:{key} 不存在", scope.as_str()));
    };

    if json_out {
        let out = json!({
            "scope": scope.as_str(),
            "key": key,
            "value": value,
            "remark": remark,
            "updated_at": ts,
        });
        println!("{}", to_json(&out)?);
    } else {
        println!("{value}");
    }
    Ok(())
}

fn cmd_set(
    db_path: &str,
    key: &str,
    value: &str,
    scope: Scope,
    remark: Option<&str>,
    json_out: bool,
) -> Result<(), String> {
    ensure_root()?;
    ensure_writable(scope)?;
    validate_key(key)?;

    let value = if value == "-" {
        read_stdin()?
    } else {
        value.to_string()
    };
    validate_value(&value)?;
    validate_known_key(key, value.trim())?;

    let conn = open(db_path)?;
    upsert(&conn, scope, key, &value, remark)?;

    if json_out {
        let out = json!({
            "ok": true,
            "action": "set",
            "scope": scope.as_str(),
            "key": key,
            "value": value,
            "remark": remark,
        });
        println!("{}", to_json(&out)?);
    } else {
        ok(&format!("已设置 {}:{key}", scope.as_str()));
        println!("{YELLOW}[!]{NC} 面板与建站流程实时读取该表，无需重启 zapd");
    }
    Ok(())
}

fn cmd_unset(db_path: &str, key: &str, scope: Scope, json_out: bool) -> Result<(), String> {
    ensure_root()?;
    ensure_writable(scope)?;
    validate_key(key)?;

    let conn = open(db_path)?;
    let n = conn
        .execute(
            "DELETE FROM server_env WHERE scope = ?1 AND k = ?2",
            params![scope.as_str(), key],
        )
        .map_err(|e| format!("删除失败: {e}"))?;

    if n == 0 {
        return Err(format!("{}:{key} 不存在", scope.as_str()));
    }

    if json_out {
        let out = json!({
            "ok": true,
            "action": "unset",
            "scope": scope.as_str(),
            "key": key,
            "removed": n,
        });
        println!("{}", to_json(&out)?);
    } else {
        ok(&format!("已删除 {}:{key}", scope.as_str()));
    }
    Ok(())
}

fn cmd_import(
    db_path: &str,
    file: &str,
    scope: Scope,
    dry_run: bool,
    skip_errors: bool,
    json_out: bool,
) -> Result<(), String> {
    ensure_writable(scope)?;
    if !dry_run {
        ensure_root()?;
    }

    let content = if file == "-" {
        read_stdin()?
    } else {
        std::fs::read_to_string(file).map_err(|e| format!("无法读取文件 {file}: {e}"))?
    };

    let mut items: Vec<(String, String)> = Vec::new();
    let mut skipped = 0usize;
    for (idx, raw) in content.lines().enumerate() {
        let lineno = idx + 1;
        let parsed = match parse_env_line(raw) {
            Ok(Some(pair)) => {
                let (k, v) = pair;
                match validate_key(k)
                    .and_then(|()| validate_value(v))
                    .and_then(|()| validate_known_key(k, v.trim()))
                {
                    Ok(()) => Ok(Some((k.to_string(), v.to_string()))),
                    Err(e) => Err(e),
                }
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        };

        match parsed {
            Ok(Some(item)) => items.push(item),
            Ok(None) => {}
            Err(e) => {
                if skip_errors {
                    skipped += 1;
                    println!("{YELLOW}[!]{NC} 跳过第 {lineno} 行：{e}");
                } else {
                    return Err(format!("第 {lineno} 行：{e}（可加 --skip-errors 跳过）"));
                }
            }
        }
    }

    if dry_run {
        for (k, v) in &items {
            println!("{k}={v}");
        }
        if json_out {
            let out = json!({
                "ok": true,
                "action": "import",
                "dry_run": true,
                "scope": scope.as_str(),
                "imported": 0,
                "skipped": skipped,
                "preview": items.iter().map(|(k, v)| json!({"key": k, "value": v})).collect::<Vec<_>>(),
            });
            println!("{}", to_json(&out)?);
        } else {
            println!(
                "{YELLOW}[!]{NC} 预览模式，未写入任何数据（共 {} 条）",
                items.len()
            );
        }
        return Ok(());
    }

    let mut conn = open(db_path)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for (k, v) in &items {
        upsert(&tx, scope, k, v, None)?;
    }
    tx.commit().map_err(|e| format!("提交失败: {e}"))?;

    if json_out {
        let out = json!({
            "ok": true,
            "action": "import",
            "dry_run": false,
            "scope": scope.as_str(),
            "imported": items.len(),
            "skipped": skipped,
        });
        println!("{}", to_json(&out)?);
    } else {
        ok(&format!(
            "已导入 {} 条到 scope={}（跳过 {} 条）",
            items.len(),
            scope.as_str(),
            skipped
        ));
    }
    Ok(())
}

// ── 工具 ──────────────────────────────────────────────────────

/// 解析一行 `key=value`；`Ok(None)` 表示空行 / 注释。
fn parse_env_line(line: &str) -> Result<Option<(&str, &str)>, String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    let line = line
        .strip_prefix("export ")
        .map(str::trim_start)
        .unwrap_or(line);
    let Some((k, v)) = line.split_once('=') else {
        return Err(format!("缺少 '='：{line}"));
    };
    let k = k.trim();
    if k.is_empty() {
        return Err(format!("键名为空：{line}"));
    }
    Ok(Some((k, strip_quotes(v.trim()))))
}

/// 去除值两端的成对引号（单 / 双引号）。
fn strip_quotes(s: &str) -> &str {
    let b = s.as_bytes();
    if b.len() >= 2 && (b[0] == b'"' || b[0] == b'\'') && b[b.len() - 1] == b[0] {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

fn read_stdin() -> Result<String, String> {
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| format!("读取标准输入失败: {e}"))?;
    Ok(buf.trim_end_matches(['\n', '\r']).to_string())
}

fn to_json(v: &Value) -> Result<String, String> {
    serde_json::to_string_pretty(v).map_err(|e| format!("序列化 JSON 失败: {e}"))
}

/// 按字符截断过长的展示字段。
fn ellipsis(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn format_time(ts: i64) -> String {
    use chrono::TimeZone;
    chrono::Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| ts.to_string())
}
