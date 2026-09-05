//! `zapctl` —— ZAP 服务器/VPS 管理系统控制工具。
//!
//! 职责：管理 `zapd`（业务主进程）与 `zapexec`（root 特权守护进程）的
//! systemd 服务（start / stop / restart / status / enable / disable / logs）；
//! 备份（backup：zap 数据库/配置、用户数据）、用户（user / passwd）、配置文件键值（config）。
//!
//! 后续运维能力（如远程任务下发、一键健康检查等）在 [`Command`] 中追加子命令即可。

mod backup;
mod config;
mod user;

use std::collections::HashMap;
use std::process::Command as ProcessCommand;

use clap::{Parser, Subcommand, ValueEnum};

// ── 服务登记表（后续新增服务在此处登记）───────────────────────
const UNIT_ZAPD: &str = "zapd.service";
const UNIT_ZAPEXEC: &str = "zapexec.service";

// ── 终端颜色（与 install.sh / rundev.sh 风格一致）─────────────
pub const GREEN: &str = "\x1b[0;32m";
pub const RED: &str = "\x1b[0;31m";
pub const YELLOW: &str = "\x1b[1;33m";
pub const BLUE: &str = "\x1b[0;34m";
pub const NC: &str = "\x1b[0m";

pub fn info(msg: &str) {
    println!("{BLUE}[*]{NC} {msg}");
}
pub fn ok(msg: &str) {
    println!("{GREEN}[✓]{NC} {msg}");
}

#[derive(Parser)]
#[command(
    name = "zapctl",
    version,
    about = "ZAP 服务器管理工具（管理 zapd / zapexec 服务）",
    long_about = None
)]
struct Cli {
    /// 覆盖数据库路径（默认从 zap.yaml 的 db.path 读取）
    #[arg(long, global = true)]
    db: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 启动服务
    Start {
        /// 目标服务，默认全部
        #[arg(value_enum, default_value_t = Service::All)]
        service: Service,
    },
    /// 停止服务
    Stop {
        #[arg(value_enum, default_value_t = Service::All)]
        service: Service,
    },
    /// 重启服务
    Restart {
        #[arg(value_enum, default_value_t = Service::All)]
        service: Service,
    },
    /// 查看服务运行状态
    Status {
        #[arg(value_enum, default_value_t = Service::All)]
        service: Service,
    },
    /// 设置开机自启
    Enable {
        #[arg(value_enum, default_value_t = Service::All)]
        service: Service,
    },
    /// 取消开机自启
    Disable {
        #[arg(value_enum, default_value_t = Service::All)]
        service: Service,
    },
    /// 查看服务日志（journalctl）
    Logs {
        #[arg(value_enum, default_value_t = Service::All)]
        service: Service,
        /// 持续跟踪日志输出（等效 journalctl -f）
        #[arg(short, long)]
        follow: bool,
        /// 显示最近 N 行
        #[arg(short = 'n', long, default_value_t = 50)]
        lines: u32,
    },
    /// 备份 / 还原 / 管理归档：zap（zap）、用户（user / users）、还原（restore）、列出（list）、清理（prune）
    Backup {
        #[command(subcommand)]
        cmd: backup::BackupCommand,
    },
    /// 用户管理
    User {
        #[command(subcommand)]
        cmd: user::UserCommand,
    },
    /// 修改指定用户的密码（校验旧密码）
    Passwd {
        /// 用户名
        username: String,
    },
    /// 查看 / 新增 / 修改 / 删除配置文件（zap.yaml）键值（不带子命令时等价于 `config get`，打印全部配置）
    Config {
        /// 目标配置文件（默认：ZAP_CONFIG > /etc/zap/zap.yaml > conf/zap.yaml）
        #[arg(short = 'f', long)]
        file: Option<String>,
        #[command(subcommand)]
        cmd: Option<config::ConfigCommand>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Service {
    /// 业务主进程
    Zapd,
    /// 特权守护进程（root）
    Zapexec,
    /// 全部服务
    All,
}

impl Service {
    fn units(self) -> Vec<&'static str> {
        match self {
            Service::Zapd => vec![UNIT_ZAPD],
            Service::Zapexec => vec![UNIT_ZAPEXEC],
            Service::All => vec![UNIT_ZAPD, UNIT_ZAPEXEC],
        }
    }
}

impl std::fmt::Display for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Service::Zapd => "zapd",
            Service::Zapexec => "zapexec",
            Service::All => "all",
        })
    }
}

fn main() {
    let cli = Cli::parse();

    let db_path = config::db_path(cli.db.as_deref());

    let result = match cli.command {
        Command::Start { service } => manage("start", service),
        Command::Stop { service } => manage("stop", service),
        Command::Restart { service } => manage("restart", service),
        Command::Enable { service } => manage("enable", service),
        Command::Disable { service } => manage("disable", service),
        Command::Status { service } => status(service),
        Command::Logs {
            service,
            follow,
            lines,
        } => logs(service, follow, lines),
        Command::Backup { cmd } => backup::dispatch(cmd, &db_path),
        Command::User { cmd } => user::dispatch(cmd, &db_path),
        Command::Passwd { username } => user::cmd_self_passwd(&db_path, &username),
        Command::Config { file, cmd } => {
            // 缺省子命令时等价于 `config get`（打印整个配置文件）
            let cmd = cmd.unwrap_or(config::ConfigCommand::Get { key: None });
            config::dispatch(cmd, file.as_deref())
        }
    };

    if let Err(e) = result {
        eprintln!("{RED}[✗]{NC} {e}");
        std::process::exit(1);
    }
}

// ── 子命令实现 ────────────────────────────────────────────────

/// 变更类操作（start/stop/restart/enable/disable）需要 root。
fn manage(verb: &str, service: Service) -> Result<(), String> {
    ensure_root()?;

    let units = service.units();
    info(&format!("systemctl {verb} {}", units.join(" ")));

    let mut args: Vec<&str> = vec![verb];
    args.extend_from_slice(&units);
    run_captured("systemctl", &args)?;

    ok(&format!("{} {}", verb, units.join(" ")));
    Ok(())
}

/// 查看状态，无需 root。
fn status(service: Service) -> Result<(), String> {
    println!(
        "{:<10} {:<24} {:<10} {:<8}",
        "SERVICE", "STATE", "ENABLED", "PID"
    );
    println!("{}", "-".repeat(10 + 1 + 24 + 1 + 10 + 1 + 8));

    for unit in service.units() {
        let name = unit.trim_end_matches(".service");
        let props = systemctl_show(unit)?;

        let load = props.get("LoadState").map(String::as_str).unwrap_or("");
        let active = props.get("ActiveState").map(String::as_str).unwrap_or("");
        let sub = props.get("SubState").map(String::as_str).unwrap_or("");
        let enabled = props.get("UnitFileState").map(String::as_str).unwrap_or("");
        let pid = props.get("MainPID").map(String::as_str).unwrap_or("");

        let (state, color) = if load == "not-found" {
            ("not-installed".to_string(), RED)
        } else if active == "active" {
            (format!("active ({sub})"), GREEN)
        } else if active == "failed" {
            ("failed".to_string(), RED)
        } else {
            (format!("{active} ({sub})"), YELLOW)
        };

        let pid = if pid.is_empty() || pid == "0" {
            "-".to_string()
        } else {
            pid.to_string()
        };
        let enabled = if enabled.is_empty() {
            "-".to_string()
        } else {
            enabled.to_string()
        };

        println!("{name:<10} {color}{state:<24}{NC} {enabled:<10} {pid:<8}");
    }

    Ok(())
}

/// 查看日志。`-f` 时使用继承 stdio 的方式以支持持续输出。
fn logs(service: Service, follow: bool, lines: u32) -> Result<(), String> {
    let mut args: Vec<String> = Vec::new();
    for unit in service.units() {
        args.push("-u".to_string());
        args.push(unit.to_string());
    }
    if follow {
        args.push("-f".to_string());
    }
    args.push("-n".to_string());
    args.push(lines.to_string());

    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
    run_inherit("journalctl", &args_ref)
}

// ── 底层工具 ──────────────────────────────────────────────────

pub fn ensure_root() -> Result<(), String> {
    let euid = unsafe { libc::geteuid() };
    if euid == 0 {
        Ok(())
    } else {
        Err("该操作需要 root 权限，请使用：sudo zapctl <命令>".to_string())
    }
}

/// 捕获输出执行；失败时返回 stderr 内容作为错误信息。
fn run_captured(program: &str, args: &[&str]) -> Result<(), String> {
    let out = ProcessCommand::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("无法执行 {program}: {e}"))?;

    if out.status.success() {
        print!("{}", String::from_utf8_lossy(&out.stdout));
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// 继承 stdio 执行（用于 journalctl -f 等需要流式/交互输出的命令）。
fn run_inherit(program: &str, args: &[&str]) -> Result<(), String> {
    let status = ProcessCommand::new(program)
        .args(args)
        .status()
        .map_err(|e| format!("无法执行 {program}: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{program} {} 执行失败（退出码 {}）",
            args.join(" "),
            status
                .code()
                .map_or_else(|| "?".to_string(), |c| c.to_string())
        ))
    }
}

/// 执行 `systemctl show -p ...`，返回 KEY=VALUE 映射（属性顺序不依赖 -p 顺序）。
fn systemctl_show(unit: &str) -> Result<HashMap<String, String>, String> {
    let out = capture(
        "systemctl",
        &[
            "show",
            "-p",
            "LoadState",
            "-p",
            "ActiveState",
            "-p",
            "SubState",
            "-p",
            "UnitFileState",
            "-p",
            "MainPID",
            unit,
        ],
    )?;

    let mut map = HashMap::new();
    for line in out.lines() {
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.to_string(), v.to_string());
        }
    }
    Ok(map)
}

/// 捕获 stdout 执行（仅成功时返回）。
fn capture(program: &str, args: &[&str]) -> Result<String, String> {
    let out = ProcessCommand::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("无法执行 {program}: {e}"))?;

    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}
