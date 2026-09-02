//! `zapctl db` 子命令：直接访问 zap.db（SQLite）。

use std::path::Path;

use clap::Subcommand;
use rusqlite::Connection;

use crate::{BLUE, NC, ok};

#[derive(Subcommand)]
pub enum DbCommand {
    /// 查看库路径、大小、表清单与行数
    Info,
    /// 列出所有表
    Tables,
    /// 备份数据库（VACUUM INTO，目标文件不可已存在）
    Backup {
        /// 备份目标文件路径
        file: std::path::PathBuf,
    },
}

pub fn dispatch(cmd: DbCommand, db_path: &str) -> Result<(), String> {
    match cmd {
        DbCommand::Info => cmd_info(db_path),
        DbCommand::Tables => cmd_tables(db_path),
        DbCommand::Backup { file } => cmd_backup(db_path, &file),
    }
}

fn open(db_path: &str) -> Result<Connection, String> {
    Connection::open(db_path).map_err(|e| format!("无法打开数据库 {db_path}: {e}"))
}

fn cmd_info(db_path: &str) -> Result<(), String> {
    let meta = std::fs::metadata(db_path).map_err(|e| format!("无法读取 {db_path}: {e}"))?;

    let conn = open(db_path)?;
    let mut tables: Vec<(String, i64)> = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            )
            .map_err(|e| e.to_string())?;
        let names: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        for name in names {
            let escaped = name.replace('"', "\"\"");
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM \"{escaped}\""), [], |r| {
                    r.get(0)
                })
                .unwrap_or(-1);
            tables.push((name, count));
        }
    }

    println!("{BLUE}数据库路径:{NC} {db_path}");
    println!("{BLUE}文件大小:{NC} {}", human_size(meta.len()));
    println!("{BLUE}表数量:{NC} {}", tables.len());
    println!();
    println!("{:<24} {:>10}", "TABLE", "ROWS");
    println!("{}", "-".repeat(36));
    for (name, count) in tables {
        println!("{name:<24} {count:>10}");
    }
    Ok(())
}

fn cmd_tables(db_path: &str) -> Result<(), String> {
    let conn = open(db_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .map_err(|e| e.to_string())?;
    let names: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    for name in names {
        println!("{name}");
    }
    Ok(())
}

fn cmd_backup(db_path: &str, dest: &Path) -> Result<(), String> {
    if dest.exists() {
        return Err(format!("目标文件已存在: {}", dest.display()));
    }
    let conn = open(db_path)?;
    let escaped = dest.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{escaped}'"))
        .map_err(|e| format!("备份失败: {e}"))?;
    ok(&format!("已备份到 {}", dest.display()));
    Ok(())
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut idx = 0;
    while value >= 1024.0 && idx < UNITS.len() - 1 {
        value /= 1024.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.2} {}", UNITS[idx])
    }
}
