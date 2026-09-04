//! ZAP 系统升级器（一次性进程，以 root 运行）。
//!
//! 由 zapexec 用 `systemd-run --no-block` 拉入独立 transient unit 执行，
//! 因此本进程**不随 zapd / zapexec 的重启而终止**，可以安全地：
//! 校验升级包 → 备份当前二进制 → 原子替换 → 依次 `systemctl restart`
//! zapexec / zapd，最后在日志中写入 `__ZAP_DONE__ <code>` 结束标记
//! （zapd 侧轮询该标记以收尾运行记录）。

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;

/// 随发行包分发的二进制清单（stage 内存在者即视为需要更新）。
const BINS: [&str; 4] = ["zapd", "zapexec", "zapctl", "zapupgrade"];

/// 与 zapd `zap::appstore::DONE_MARKER` 保持一致的日志结束标记。
const DONE_MARKER: &str = "__ZAP_DONE__";

#[derive(Parser, Debug)]
#[command(
    name = "zapupgrade",
    about = "ZAP 系统升级器（一次性，root 运行）",
    version
)]
struct Cli {
    /// zapd 下载解包并规整后的 stage 目录（含新二进制与 version 文件）
    #[arg(long)]
    stage: String,
    /// ZAP 安装根目录（zapexec 传入，生产为 /usr/local/zap）
    #[arg(long, default_value = "/usr/local/zap")]
    dir: String,
    /// 升级日志文件（追加写，zapd 通过轮询展示进度）
    #[arg(long)]
    log: String,
}

fn log_line(log_path: &str, s: &str) {
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        let _ = writeln!(f, "{s}");
    }
}

fn systemctl(args: &[&str]) -> bool {
    std::process::Command::new("systemctl")
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 目标服务是否正由 systemd 加载管理？
/// 开发/容器环境（rundev.sh 裸进程、docker）没有 systemd unit，返回 false。
fn unit_managed(unit: &str) -> bool {
    if !std::path::Path::new("/run/systemd/system").exists() {
        return false;
    }
    std::process::Command::new("systemctl")
        .args(["show", unit, "--property=Id", "--no-pager"])
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains("Id="))
        .unwrap_or(false)
}

fn main() {
    let cli = Cli::parse();
    log_line(&cli.log, "==== ZAP 系统升级开始 ====");
    let code = run_upgrade(&cli);
    log_line(&cli.log, &format!("{DONE_MARKER} {code}"));
    std::process::exit(code);
}

fn run_upgrade(cli: &Cli) -> i32 {
    let dir = PathBuf::from(&cli.dir);
    let stage = PathBuf::from(&cli.stage);

    // 1) 目标版本（zapd 下载时写入 stage/version）
    let version = fs::read_to_string(stage.join("version"))
        .unwrap_or_default()
        .trim()
        .to_string();
    log_line(&cli.log, &format!("目标版本: {version}"));

    // 2) 待更新二进制 = stage 中存在的清单项
    let mut targets: Vec<&str> = Vec::new();
    for b in BINS {
        if stage.join(b).is_file() {
            targets.push(b);
        }
    }
    if targets.is_empty() {
        log_line(&cli.log, "升级包内没有可更新的二进制，中止");
        return 1;
    }
    log_line(&cli.log, &format!("待更新二进制: {}", targets.join(", ")));

    // 3) 备份当前二进制到 data/upgrade/backup/{ts}-{version}/
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup_dir = dir
        .join("data/upgrade/backup")
        .join(format!("{ts}-{version}"));
    if fs::create_dir_all(&backup_dir).is_err() {
        log_line(&cli.log, "创建备份目录失败");
        return 1;
    }
    let mut backed_up: Vec<&str> = Vec::new();
    for b in &targets {
        let cur = dir.join(b);
        if cur.exists() {
            match fs::copy(&cur, backup_dir.join(b)) {
                Ok(_) => {
                    backed_up.push(b);
                    log_line(&cli.log, &format!("已备份 {b}"));
                }
                Err(e) => {
                    log_line(&cli.log, &format!("备份 {b} 失败: {e}，中止"));
                    return 1;
                }
            }
        }
    }

    // 4) 原子替换（先写临时文件再 rename，避免半写状态）
    for b in &targets {
        let tmp = dir.join(format!(".{b}.upgrade-new"));
        let dst = dir.join(b);
        if let Err(e) = fs::copy(stage.join(b), &tmp) {
            let _ = fs::remove_file(&tmp);
            log_line(&cli.log, &format!("写入 {b} 失败: {e}，中止"));
            return 1;
        }
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755));
        if let Err(e) = fs::rename(&tmp, &dst) {
            let _ = fs::remove_file(&tmp);
            log_line(&cli.log, &format!("替换 {b} 失败: {e}，中止"));
            return 1;
        }
        log_line(&cli.log, &format!("已替换 {b}"));
    }

    // 5) 重启受影响的进程：zapexec 先、zapd 后（zapd 最后保证面板进程即最新版本）。
    //    仅对 systemd 托管的服务执行自动 restart（生产环境）；
    //    开发/容器环境（rundev.sh 裸进程、docker）二进制已替换成功，
    //    记录为「需手动重启」而不是失败回滚。
    let mut failed: Vec<String> = Vec::new();
    let mut manual: Vec<String> = Vec::new();
    for svc in ["zapexec", "zapd"] {
        if !targets.contains(&svc) {
            continue;
        }
        let unit = format!("{svc}.service");
        if !unit_managed(&unit) {
            log_line(
                &cli.log,
                &format!(
                    "{unit} 不受 systemd 管理（开发/容器环境），新二进制已替换，请手动重启 {svc} 生效"
                ),
            );
            manual.push(svc.to_string());
            continue;
        }
        if systemctl(&["restart", &unit]) {
            log_line(&cli.log, &format!("{unit} 已重启"));
        } else {
            log_line(&cli.log, &format!("重启 {unit} 失败，尝试回滚 {svc}"));
            failed.push(svc.to_string());
        }
    }

    // 6) 回滚：对重启失败的服务恢复备份并再试一次
    for svc in &failed {
        if let Some(src) = backup_dir.join(svc).exists().then(|| backup_dir.join(svc)) {
            let dst = dir.join(svc);
            if fs::copy(&src, &dst).is_ok() {
                let _ = fs::set_permissions(&dst, fs::Permissions::from_mode(0o755));
                log_line(&cli.log, &format!("已回滚 {svc} 到旧版本"));
            }
        }
        let unit = format!("{svc}.service");
        if systemctl(&["restart", &unit]) {
            log_line(&cli.log, &format!("回滚后 {unit} 已重启"));
        } else {
            log_line(&cli.log, &format!("{unit} 回滚后仍无法启动，请人工介入"));
        }
    }

    if failed.is_empty() {
        if manual.is_empty() {
            log_line(&cli.log, "升级完成");
        } else {
            log_line(
                &cli.log,
                &format!("升级完成（{} 需手动重启后生效）", manual.join(", ")),
            );
        }
        0
    } else {
        log_line(
            &cli.log,
            &format!("升级未完全成功（失败服务: {}）", failed.join(", ")),
        );
        1
    }
}
