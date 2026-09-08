//! 通用服务配置（root 执行）：「服务配置」大类下 Nginx 之外的各服务
//! （php / mysql / mariadb / docker …）的配置页后端能力。
//!
//! 设计目标：新增一个服务 = 在 `supported()` 中注册一条 `ServiceDef`，
//! 即获得「状态探测 / 启停控制 / 配置文件读写 / 关键项可视化」四件套，
//! 无需为每个服务重写 verb（Nginx 因校验/重载链复杂仍使用专用 verb nginx.rs）。
//!
//! 端点（均需管理员，见 zapd/system_service_conf.rs）：
//! - service_conf.status      状态探测（未安装时 installed=false）
//! - service_conf.list        列出可编辑配置（主配置 + 配置目录白名单）
//! - service_conf.read        读取指定配置内容
//! - service_conf.save        保存配置（备份 → 原子写入；json 校验）
//! - service_conf.keys        关键项表单定义 + 当前值
//! - service_conf.keys_save   关键项保存（ini 托管块 / json 键合并）
//! - service_conf.control     服务控制 start / stop / restart / reload
//!
//! 安全边界：文件路径必须位于该服务主配置所在目录树内且扩展名白名单；
//! 不执行任何用户输入命令，只运行服务定义内置的命令模板。

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use zap_proto::Response;

use super::root_cmd;

/// 单个配置文件体积上限（读 / 写通用）。
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
/// 从主配置目录树内收集文件的最大深度。
const SCAN_DEPTH: usize = 2;
/// 服务控制允许的动作。
const ALLOWED_ACTIONS: &[&str] = &["start", "stop", "restart", "reload"];

// ── 服务定义 ─────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConfFormat {
    /// ini 风格：key = value（php.ini 无 section / my.cnf 有 [section]）
    Ini,
    /// JSON 对象（docker daemon.json）
    Json,
}

#[derive(Clone, Copy)]
enum FieldKind {
    Text,
    Number,
    Select,
    Bool,
}

struct FieldDef {
    key: &'static str,
    label: &'static str,
    kind: FieldKind,
    help: &'static str,
    /// ini：写入的 section（None = 文件层/无 section）
    section: Option<&'static str>,
    /// json：逐级键路径（如 ["log-driver"] / ["log-opts","max-size"]）
    jpath: &'static [&'static str],
    options: &'static [&'static str],
}

struct ServiceDef {
    key: &'static str,
    label: &'static str,
    /// systemd unit 名候选（依次取第一个存在的）
    unit_candidates: &'static [&'static str],
    /// 探测二进制的可执行文件名候选
    bin_candidates: &'static [&'static str],
    /// 取版本时二进制后追加的参数（留空 = `--version`）
    version_args: &'static [&'static str],
    /// 版本输出落在 stderr（如 nginx -v；本模块服务多为 stdout）
    version_in_stderr: bool,
    /// 主配置文件候选（可含一个 `*`，用于版本目录），依次取第一个存在的；
    /// 全部不存在但服务已安装时取首个不含通配的候选（允许从 UI 新建）
    main_candidates: &'static [&'static str],
    /// 可编辑文件的扩展名白名单（不含点）
    exts: &'static [&'static str],
    format: ConfFormat,
    /// ini 注释符（my.cnf 用 #，php.ini 用 ;）
    ini_comment: &'static str,
    fields: &'static [FieldDef],
}

const PHP_FIELDS: &[FieldDef] = &[
    FieldDef {
        key: "memory_limit",
        label: "memory_limit",
        kind: FieldKind::Text,
        help: "单个 PHP 进程可用内存上限，如 128M / 256M / 512M",
        section: None,
        jpath: &[],
        options: &[],
    },
    FieldDef {
        key: "upload_max_filesize",
        label: "upload_max_filesize",
        kind: FieldKind::Text,
        help: "上传文件大小上限，如 20M / 50M（需同时放大 post_max_size）",
        section: None,
        jpath: &[],
        options: &[],
    },
    FieldDef {
        key: "post_max_size",
        label: "post_max_size",
        kind: FieldKind::Text,
        help: "POST 数据大小上限，建议略大于 upload_max_filesize",
        section: None,
        jpath: &[],
        options: &[],
    },
    FieldDef {
        key: "max_execution_time",
        label: "max_execution_time",
        kind: FieldKind::Number,
        help: "单个脚本最大执行时间（秒），CLI 默认不受限",
        section: None,
        jpath: &[],
        options: &[],
    },
    FieldDef {
        key: "max_input_time",
        label: "max_input_time",
        kind: FieldKind::Number,
        help: "解析输入数据的最长时间（秒）",
        section: None,
        jpath: &[],
        options: &[],
    },
    FieldDef {
        key: "date.timezone",
        label: "date.timezone",
        kind: FieldKind::Text,
        help: "时区，如 Asia/Shanghai",
        section: None,
        jpath: &[],
        options: &[],
    },
    FieldDef {
        key: "display_errors",
        label: "display_errors",
        kind: FieldKind::Select,
        help: "是否把错误输出到页面（生产环境建议 Off）",
        section: None,
        jpath: &[],
        options: &["Off", "On"],
    },
    FieldDef {
        key: "opcache.enable",
        label: "opcache.enable",
        kind: FieldKind::Select,
        help: "是否启用 opcache（PHP >= 5.5 内置）",
        section: None,
        jpath: &[],
        options: &["1", "0"],
    },
];

const MYSQL_FIELDS: &[FieldDef] = &[
    FieldDef {
        key: "port",
        label: "port",
        kind: FieldKind::Number,
        help: "监听端口，默认 3306",
        section: Some("mysqld"),
        jpath: &[],
        options: &[],
    },
    FieldDef {
        key: "bind-address",
        label: "bind-address",
        kind: FieldKind::Text,
        help: "监听地址：127.0.0.1 仅本机；0.0.0.0 对外（请配合防火墙）",
        section: Some("mysqld"),
        jpath: &[],
        options: &[],
    },
    FieldDef {
        key: "max_connections",
        label: "max_connections",
        kind: FieldKind::Number,
        help: "最大并发连接数，建议 200-2000",
        section: Some("mysqld"),
        jpath: &[],
        options: &[],
    },
    FieldDef {
        key: "character-set-server",
        label: "character-set-server",
        kind: FieldKind::Select,
        help: "默认字符集",
        section: Some("mysqld"),
        jpath: &[],
        options: &["utf8mb4", "utf8", "latin1"],
    },
    FieldDef {
        key: "collation-server",
        label: "collation-server",
        kind: FieldKind::Text,
        help: "默认排序规则，utf8mb4 建议 utf8mb4_unicode_ci",
        section: Some("mysqld"),
        jpath: &[],
        options: &[],
    },
    FieldDef {
        key: "innodb_buffer_pool_size",
        label: "innodb_buffer_pool_size",
        kind: FieldKind::Text,
        help: "InnoDB 缓冲池大小，约为内存的 50%-70%，如 1G / 4G",
        section: Some("mysqld"),
        jpath: &[],
        options: &[],
    },
];

const DOCKER_FIELDS: &[FieldDef] = &[
    FieldDef {
        key: "log_driver",
        label: "日志驱动",
        kind: FieldKind::Select,
        help: "容器日志驱动（logging driver）",
        section: None,
        jpath: &["log-driver"],
        options: &["json-file", "local", "journald", "none", "syslog"],
    },
    FieldDef {
        key: "log_max_size",
        label: "单容器日志上限",
        kind: FieldKind::Text,
        help: "json-file/local 驱动的单文件大小上限，如 20m / 100m",
        section: None,
        jpath: &["log-opts", "max-size"],
        options: &[],
    },
    FieldDef {
        key: "data_root",
        label: "数据目录",
        kind: FieldKind::Text,
        help: "Docker 数据根目录（默认 /var/lib/docker），修改需迁移数据",
        section: None,
        jpath: &["data-root"],
        options: &[],
    },
    FieldDef {
        key: "debug",
        label: "调试日志",
        kind: FieldKind::Bool,
        help: "是否开启 dockerd 调试日志",
        section: None,
        jpath: &["debug"],
        options: &["true", "false"],
    },
    FieldDef {
        key: "icc",
        label: "容器间互联 icc",
        kind: FieldKind::Bool,
        help: "默认 bridge 网络上容器是否可互相通信",
        section: None,
        jpath: &["icc"],
        options: &["true", "false"],
    },
];

/// PHP 类型服务的静态定义（type 级 "php"，面向系统包安装）。
const PHP_DEF: ServiceDef = ServiceDef {
    key: "php",
    label: "PHP",
    unit_candidates: &["php-fpm"],
    bin_candidates: &["php-fpm", "php"],
    version_args: &["-v"],
    version_in_stderr: false,
    main_candidates: &["/etc/php/*/fpm/php.ini", "/etc/php.ini"],
    exts: &["ini"],
    format: ConfFormat::Ini,
    ini_comment: ";",
    fields: PHP_FIELDS,
};

/// PHP 实例短名识别：svc = "php" + 版本号数字（php74 / php81 …，
/// 对应应用商店实例安装目录 `{ZAP_APPS_DIR}/php-74` 与 unit
/// `php-fpm-74`）。多版本实例的配置路径/unit 与类型级 "php" 不同。
fn php_inst_svc(svc: &str) -> Option<String> {
    let rest = svc.strip_prefix("php")?;
    if rest.is_empty() || rest.len() > 3 || !rest.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(rest.to_string())
}

/// 服务注册表：新增服务在此追加即可（key 需与前端菜单/API 一致）。
/// php<版本号>（php74 / php81 …）作为 PHP 实例 svc 一并识别，复用 PHP_DEF
/// 的字段 / 白名单 / 关键项，探测与路径在 `php_inst` / `installed_info` 覆写。
fn supported(key: &str) -> Option<&'static ServiceDef> {
    if php_inst_svc(key).is_some() {
        return Some(&PHP_DEF);
    }
    Some(match key {
        "php" => &PHP_DEF,
        "mysql" => &ServiceDef {
            key: "mysql",
            label: "MySQL",
            unit_candidates: &["mysql", "mysqld"],
            bin_candidates: &["mysqld", "mysql"],
            version_args: &["--version"],
            version_in_stderr: false,
            main_candidates: &["/etc/mysql/my.cnf", "/etc/my.cnf"],
            exts: &["cnf", "conf"],
            format: ConfFormat::Ini,
            ini_comment: "#",
            fields: MYSQL_FIELDS,
        },
        "mariadb" => &ServiceDef {
            key: "mariadb",
            label: "MariaDB",
            unit_candidates: &["mariadb", "mysql"],
            bin_candidates: &["mariadbd", "mysqld", "mysql"],
            version_args: &["--version"],
            version_in_stderr: false,
            main_candidates: &[
                "/etc/mysql/mariadb.conf.d/99-zap.cnf",
                "/etc/mysql/my.cnf",
                "/etc/my.cnf",
            ],
            exts: &["cnf", "conf"],
            format: ConfFormat::Ini,
            ini_comment: "#",
            fields: MYSQL_FIELDS,
        },
        "docker" => &ServiceDef {
            key: "docker",
            label: "Docker",
            unit_candidates: &["docker"],
            bin_candidates: &["dockerd", "docker"],
            version_args: &["--version"],
            version_in_stderr: false,
            main_candidates: &["/etc/docker/daemon.json"],
            exts: &["json"],
            format: ConfFormat::Json,
            ini_comment: "#",
            fields: DOCKER_FIELDS,
        },
        _ => return None,
    })
}

// ── 探测工具 ─────────────────────────────────────────────────

fn zap_path() -> PathBuf {
    std::env::var("ZAP_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/usr/local/zap"))
}

fn quote_shell(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// `command -v name` 找到可执行文件。
fn which(name: &str) -> Option<PathBuf> {
    let o = root_cmd("bash")
        .args(["-c"])
        .arg(format!("command -v {} 2>/dev/null", quote_shell(name)))
        .output()
        .ok()?;
    if !o.status.success() {
        return None;
    }
    let p = String::from_utf8_lossy(&o.stdout).trim().to_string();
    if p.is_empty() {
        return None;
    }
    Some(PathBuf::from(p))
}

/// 依次探测二进制：PATH 内 → 常见安装前缀（覆盖 /usr/local/apps 型安装）。
fn find_bin(d: &ServiceDef) -> Option<PathBuf> {
    for name in d.bin_candidates {
        if let Some(p) = which(name) {
            return Some(p);
        }
    }
    // 安装根（默认 /usr/local/apps，ZAP_APPS_DIR 可覆盖）下常见位置
    let install_root = super::install_root();
    for name in d.bin_candidates {
        for prefix in [
            install_root.clone(),
            install_root.join(d.key),
            PathBuf::from(format!("/usr/local/{}", d.key)),
        ] {
            for sub in ["bin", "sbin"] {
                let p = prefix.join(sub).join(name);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// 取版本号（首个二进制 + 固定参数的首行输出）。
fn detect_version(d: &ServiceDef, bin: &Path) -> String {
    let mut cmd = root_cmd(
        bin.to_str()
            .unwrap_or(bin.file_name().and_then(|n| n.to_str()).unwrap_or("")),
    );
    for a in d.version_args {
        cmd.arg(a);
    }
    if d.version_args.is_empty() {
        cmd.arg("--version");
    }
    let o = cmd.output();
    let text = match o {
        Ok(o) => {
            let out = if d.version_in_stderr || o.stdout.is_empty() {
                String::from_utf8_lossy(&o.stderr)
            } else {
                String::from_utf8_lossy(&o.stdout)
            };
            out.trim().to_string()
        }
        Err(_) => String::new(),
    };
    text.lines().next().unwrap_or("").trim().to_string()
}

/// systemd 是否存在名为 `name` 的 unit。
fn systemd_has(name: &str) -> bool {
    root_cmd("systemctl")
        .args(["list-unit-files", "--no-legend", &format!("{name}.service")])
        .output()
        .map(|o| {
            let out = String::from_utf8_lossy(&o.stdout);
            out.lines().any(|l| l.trim().starts_with(name))
        })
        .unwrap_or(false)
}

/// unit 当前是否 active。
fn systemd_active(name: &str) -> bool {
    root_cmd("systemctl")
        .args(["is-active", name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 主配置探测：候选含一个 `*` 时按首个存在的展开；否则取第一个存在的文件。
fn probe_main(d: &ServiceDef) -> Option<(PathBuf, PathBuf, bool)> {
    let mut first_plain: Option<&str> = None;
    for cand in d.main_candidates {
        if cand.contains('*') {
            if let Some(p) = glob_first(cand) {
                return Some((
                    p.clone(),
                    p.parent().unwrap_or(Path::new("/")).to_path_buf(),
                    true,
                ));
            }
            continue;
        }
        if first_plain.is_none() {
            first_plain = Some(cand);
        }
        let p = PathBuf::from(cand);
        if p.is_file() {
            return Some((
                p.clone(),
                p.parent().unwrap_or(Path::new("/")).to_path_buf(),
                true,
            ));
        }
    }
    // 全部不存在：服务已安装时允许回退到首个不含通配的候选（UI 可新建）
    if let Some(cand) = first_plain {
        let p = PathBuf::from(cand);
        if !cand.contains('*') {
            let dir = p.parent().unwrap_or(Path::new("/")).to_path_buf();
            return Some((p, dir, false));
        }
    }
    None
}

/// 支持单个 `*` 的极简 glob：展开目录名匹配 `base/*/suffix`。
fn glob_first(pattern: &str) -> Option<PathBuf> {
    let (head, tail) = pattern.split_once('*')?;
    let base = Path::new(head);
    let dir = base.parent().unwrap_or(Path::new("/"));
    let prefix = base.file_name()?.to_string_lossy().to_string();
    let mut rd = std::fs::read_dir(dir).ok()?;
    let mut candidates: Vec<PathBuf> = Vec::new();
    while let Some(Ok(e)) = rd.next() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with(&prefix) && e.path().is_dir() {
            let p = e.path().join(tail.trim_start_matches('/'));
            if p.is_file() {
                candidates.push(p);
            }
        }
    }
    candidates.sort();
    candidates.into_iter().next()
}

/// 通过 /proc 扫描判断是否存在 cmdline 包含二进制名的进程。
fn running_by_proc(bin: &Path) -> bool {
    let needle = bin
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();
    if needle.is_empty() {
        return false;
    }
    let Ok(rd) = std::fs::read_dir("/proc") else {
        return false;
    };
    for e in rd.flatten() {
        let name = e.file_name();
        let Ok(pid) = name.to_string_lossy().parse::<u32>() else {
            continue;
        };
        let cmdline = std::fs::read_to_string(format!("/proc/{pid}/cmdline")).unwrap_or_default();
        if cmdline.contains(&needle) {
            return true;
        }
    }
    false
}

/// PHP 实例探测上下文：由 svc（php74）定位到安装目录。
struct PhpInst {
    digits: String,
    dir: PathBuf,
}

/// PHP 实例解析：svc=php74 → 安装目录 `{ZAP_APPS_DIR}/php-74`（不存在返回 None）。
fn php_inst(svc: &str) -> Option<PhpInst> {
    let digits = php_inst_svc(svc)?;
    let dir = super::install_root().join(format!("php-{digits}"));
    dir.is_dir().then_some(PhpInst { digits, dir })
}

/// 候选探测：取第一个存在的 systemd unit 名（无 .service 后缀）。
fn active_unit(d: &ServiceDef) -> Option<String> {
    d.unit_candidates
        .iter()
        .find(|u| systemd_has(u))
        .map(|u| u.to_string())
}

/// 实际使用的 systemd unit 名（无 .service 后缀）。
/// PHP 实例 svc 使用实例 unit `php-fpm-<ver>`；其余走候选探测。
fn effective_unit(d: &ServiceDef, svc: &str) -> Option<String> {
    if let Some(inst) = php_inst(svc) {
        let u = format!("php-fpm-{}", inst.digits);
        return systemd_has(&u).then_some(u);
    }
    active_unit(d)
}

/// 服务当前运行态。
fn service_running(d: &ServiceDef, svc: &str, bin: Option<&Path>) -> bool {
    if let Some(unit) = effective_unit(d, svc) {
        if systemd_active(&unit) {
            return true;
        }
        // unit 存在但未 active 即未运行（不继续看进程，避免误判）
        return false;
    }
    if let Some(bin) = bin {
        return running_by_proc(bin);
    }
    false
}

// ── 文件操作 ─────────────────────────────────────────────────

/// 服务配置备份目录：`{ZAP_PATH}/data/backups/<svc>`（svc 为实例名，如
/// php74 / php81 / mysql 等）。统一约定：nginx 与各服务应用的备份共用
/// `data/backups/` 根目录，每服务一个子目录，便于统一浏览与容量管理。
fn backup_dir(svc: &str) -> PathBuf {
    zap_path().join("data/backups").join(svc)
}

fn backup_file(svc: &str, path: &Path) -> Result<PathBuf, String> {
    let dir = backup_dir(svc);
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建备份目录失败: {e}"))?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("conf");
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dest = dir.join(format!("{ts}-{name}.bak"));
    if path.is_file() {
        std::fs::copy(path, &dest).map_err(|e| format!("备份失败: {e}"))?;
    }
    let mut kept: Vec<PathBuf> = std::fs::read_dir(&dir)
        .ok()
        .map(|rd| rd.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    kept.sort();
    while kept.len() > 20 {
        if let Some(old) = kept.first() {
            let _ = std::fs::remove_file(old);
            kept.remove(0);
        }
    }
    Ok(dest)
}

/// 校验待编辑路径：必须位于主配置目录树内且扩展名白名单。
fn validate_path(
    d: &ServiceDef,
    main: &Path,
    root: &Path,
    raw: &str,
) -> Result<(PathBuf, bool), String> {
    let main_canon = main.canonicalize().unwrap_or_else(|_| main.to_path_buf());
    let p = PathBuf::from(raw);
    let canon = p.canonicalize().unwrap_or_else(|_| p.clone());
    let is_main = canon == main_canon;
    if !canon.starts_with(root) {
        return Err(format!(
            "仅允许编辑 {} 主配置目录（{}）内的配置文件",
            d.label,
            root.display()
        ));
    }
    let fname = canon
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();
    let ok_ext = d.exts.iter().any(|e| fname.ends_with(&format!(".{e}")));
    if !ok_ext {
        return Err(format!(
            "仅支持编辑 {}",
            d.exts
                .iter()
                .map(|e| format!(".{e}"))
                .collect::<Vec<_>>()
                .join(" / ")
        ));
    }
    if fname.starts_with("zap-")
        || fname.ends_with(".bak")
        || fname.ends_with(".tmp")
        || fname.contains(".zap")
    {
        return Err("该文件为面板托管/备份文件，请勿直接编辑".to_string());
    }
    Ok((canon, is_main))
}

fn file_entry(path: &Path, base: &Path, is_main: bool, size: u64, exists: bool) -> Value {
    let rel = path
        .strip_prefix(base)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string());
    let mtime = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    json!({
        "path": path.display().to_string(),
        "rel": rel,
        "name": path.file_name().and_then(|n| n.to_str()).unwrap_or_default(),
        "is_main": is_main,
        "size": size,
        "mtime": mtime,
        "exists": exists,
    })
}

fn collect_files(d: &ServiceDef, root: &Path, out: &mut Vec<Value>) {
    fn walk(d: &ServiceDef, dir: &Path, base: &Path, depth: usize, out: &mut Vec<Value>) {
        if depth > SCAN_DEPTH {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                !p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with('.'))
            })
            .collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                walk(d, &p, base, depth + 1, out);
                continue;
            }
            let fname = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            let ok_ext = d.exts.iter().any(|e| fname.ends_with(&format!(".{e}")));
            if !ok_ext
                || fname.starts_with("zap-")
                || fname.ends_with(".bak")
                || fname.ends_with(".tmp")
                || fname.contains(".zap")
            {
                continue;
            }
            if let Ok(meta) = std::fs::metadata(&p) {
                if meta.len() > MAX_FILE_BYTES {
                    continue;
                }
                out.push(file_entry(&p, base, false, meta.len(), true));
            }
        }
    }
    walk(d, root, root, 0, out);
}

// ── ini 关键项托管 ───────────────────────────────────────────

/// 生成托管块起始 / 结束标记行。
fn marker_begin(comment: &str, svc: &str) -> String {
    format!("{comment} ==== zap-managed ({svc}) begin ====")
}
fn marker_end(comment: &str, svc: &str) -> String {
    format!("{comment} ==== zap-managed ({svc}) end ====")
}

/// 从文本中剔除旧的托管块（含标记行），返回清理后的文本。
fn strip_managed_block(content: &str, comment: &str, svc: &str) -> String {
    let begin = marker_begin(comment, svc);
    let end = marker_end(comment, svc);
    let mut out = String::new();
    let mut skipping = false;
    for line in content.lines() {
        if line.trim_start().starts_with(&begin) {
            skipping = true;
            continue;
        }
        if skipping && line.trim_start().starts_with(&end) {
            skipping = false;
            continue;
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// ini 块内容行（key = value）。注释符与 `strip_managed_block` / 读取逻辑保持一致。
fn block_lines(comment: &str, svc: &str, entries: &[(&str, String)]) -> String {
    let mut s = String::new();
    s.push_str(&marker_begin(comment, svc));
    s.push('\n');
    for (k, v) in entries {
        s.push_str(&format!("{k} = {v}\n"));
    }
    s.push_str(&marker_end(comment, svc));
    s.push('\n');
    s
}

/// 把托管块插入到指定 [section] 内的末尾（该 section 内后续出现的同键才会覆盖托管值，
/// 因此放在 section 末尾以尽量保证托管值生效）。
fn insert_block(content: &str, section: Option<&str>, block: &str) -> String {
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    if let Some(sec) = section {
        let header = format!("[{sec}]");
        // 定位 section 起始行
        let start = lines
            .iter()
            .position(|l| l.trim().eq_ignore_ascii_case(&header));
        let Some(start) = start else {
            // 无该 section：末尾补 section + 块
            let mut s = String::new();
            if !content.is_empty() {
                s.push('\n');
            }
            s.push_str(&header);
            s.push('\n');
            s.push_str(block);
            return s;
        };
        // 该 section 内末尾 = 下一个 [ 起始行前
        let end = lines[start + 1..]
            .iter()
            .position(|l| l.trim_start().starts_with('['));
        let insert_at = match end {
            Some(rel) => start + 1 + rel,
            None => lines.len(),
        };
        // 清理 section 内部与末尾的空行，避免插入位置前堆积空行
        let block_owned: Vec<String> = block.lines().map(|l| l.to_string()).collect();
        lines.splice(insert_at..insert_at, block_owned);
        let mut out = lines.join("\n");
        out.push('\n');
        return out;
    }
    // 无 section（php.ini 等）：整体追加到文件末尾
    let mut out = String::new();
    if !content.is_empty() {
        out.push_str(content.trim_end());
        out.push('\n');
    }
    out.push_str(block);
    out
}

/// 从 ini 文本中读取 key 值（优先托管块内，其次全文最后一次未注释赋值）。
fn ini_read_values(
    content: &str,
    comment: &str,
    svc: &str,
    fields: &[FieldDef],
) -> std::collections::BTreeMap<String, Value> {
    let mut values = std::collections::BTreeMap::new();
    // 1) 先取托管块内赋值
    let begin = marker_begin(comment, svc);
    let end = marker_end(comment, svc);
    let mut in_block = false;
    let mut in_block_values: std::collections::BTreeMap<String, String> = Default::default();
    for line in content.lines() {
        let t = line.trim_start();
        if t.starts_with(&begin) {
            in_block = true;
            continue;
        }
        if in_block && t.starts_with(&end) {
            in_block = false;
            continue;
        }
        if in_block && let Some((k, v)) = parse_ini_line(t) {
            in_block_values.insert(k, v);
        }
    }
    // 2) 全文扫描未注释赋值，记最后出现的值作为回退
    let mut fallback: std::collections::BTreeMap<String, String> = Default::default();
    for line in content.lines() {
        let t = line.trim_start();
        if t.is_empty() || t.starts_with(comment) || t.starts_with('#') || t.starts_with(';') {
            continue;
        }
        if let Some((k, v)) = parse_ini_line(t) {
            fallback.insert(k, v);
        }
    }
    for f in fields {
        let v = in_block_values.get(f.key).or_else(|| fallback.get(f.key));
        values.insert(
            f.key.to_string(),
            v.cloned().map_or(Value::Null, Value::String),
        );
    }
    values
}

fn parse_ini_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    // key[空格]*=[空格]*value
    let eq = line.find('=')?;
    let key = line[..eq].trim().to_string();
    if key.is_empty() {
        return None;
    }
    let val = line[eq + 1..]
        .trim()
        .trim_end_matches(';')
        .trim()
        .to_string();
    Some((key, val))
}

// ── json 关键项 ──────────────────────────────────────────────

fn json_read_values(
    content: &str,
    fields: &[FieldDef],
) -> std::collections::BTreeMap<String, Value> {
    let mut values = std::collections::BTreeMap::new();
    let parsed: Value = serde_json::from_str(content.trim()).unwrap_or(Value::Null);
    for f in fields {
        let mut cur = &parsed;
        let mut found = true;
        for seg in f.jpath {
            match cur.get(*seg) {
                Some(v) => cur = v,
                None => {
                    found = false;
                    break;
                }
            }
        }
        if found && !cur.is_null() {
            values.insert(f.key.to_string(), cur.clone());
        } else {
            values.insert(f.key.to_string(), Value::Null);
        }
    }
    values
}

fn set_json_path(obj: &mut Value, path: &[&str], value: Value) {
    if path.is_empty() {
        return;
    }
    if !obj.is_object() {
        *obj = json!({});
    }
    let o = obj.as_object_mut().expect("object");
    if path.len() == 1 {
        o.insert(path[0].to_string(), value);
        return;
    }
    let entry = o.entry(path[0].to_string()).or_insert_with(|| json!({}));
    set_json_path(entry, &path[1..], value);
}

// ── verbs ────────────────────────────────────────────────────

async fn run_blocking<F>(f: F) -> Response
where
    F: FnOnce() -> Result<Response, String> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
        .unwrap_or_else(|e| Response::err(-1, e))
}

/// installed_info 返回的组装信息（status / list 共用）。
type InstalledInfo = (
    Option<PathBuf>,
    Option<String>,
    Option<(PathBuf, PathBuf, bool)>,
);

/// 组装常见安装信息（status / list 共用）。
/// PHP 实例 svc（php74 / php81 …）直接定位安装目录内路径与实例 unit：
/// bin = {dir}/bin/php、主配置 = {dir}/etc/php.ini（目录 etc 为可编辑根）。
fn installed_info(d: &ServiceDef, svc: &str) -> InstalledInfo {
    if let Some(inst) = php_inst(svc) {
        let bin = inst.dir.join("bin/php");
        let bin = bin.is_file().then_some(bin);
        let unit = effective_unit(d, svc);
        let main = if bin.is_some() {
            let m = inst.dir.join("etc/php.ini");
            let exists = m.is_file();
            let root = m.parent().unwrap_or(&inst.dir).to_path_buf();
            Some((m, root, exists))
        } else {
            None
        };
        return (bin, unit, main);
    }
    let bin = find_bin(d);
    let unit = active_unit(d);
    let installed = bin.is_some() || unit.is_some();
    let main = if installed { probe_main(d) } else { None };
    (bin, unit, main)
}

/// service_conf.status
pub async fn status(svc: &str) -> Response {
    let svc = svc.to_string();
    run_blocking(move || {
        let Some(d) = supported(&svc) else {
            return Err(format!("不支持的服务类型: {svc}"));
        };
        let (bin, unit, main) = installed_info(d, &svc);
        if bin.is_none() && unit.is_none() {
            return Ok(Response::ok(
                "ok",
                Some(json!({ "installed": false, "service": svc })),
            ));
        }
        let version = bin
            .as_deref()
            .map(|b| detect_version(d, b))
            .unwrap_or_default();
        let (conf_file, conf_dir, main_exists) = match &main {
            Some((m, dir, exists)) => (
                Some(m.display().to_string()),
                Some(dir.display().to_string()),
                *exists,
            ),
            None => (None, None, false),
        };
        let running = service_running(d, &svc, bin.as_deref());
        let bin_path = bin.as_deref().map(|b| b.display().to_string());
        Ok(Response::ok(
            "ok",
            Some(json!({
                "installed": true,
                "service": svc,
                "label": d.label,
                "bin": bin_path,
                "version": version,
                "unit": unit,
                "running": running,
                "systemd": unit.is_some(),
                "conf_file": conf_file,
                "conf_dir": conf_dir,
                "main_exists": main_exists,
            })),
        ))
    })
    .await
}

/// service_conf.list
pub async fn conf_list(svc: &str) -> Response {
    let svc = svc.to_string();
    run_blocking(move || {
        let Some(d) = supported(&svc) else {
            return Err(format!("不支持的服务类型: {svc}"));
        };
        let (bin, unit, main) = installed_info(d, &svc);
        if bin.is_none() && unit.is_none() {
            return Ok(Response::ok(
                "ok",
                Some(json!({ "installed": false, "files": [] })),
            ));
        }
        let Some((main, root, main_exists)) = main else {
            return Ok(Response::ok(
                "ok",
                Some(json!({
                    "installed": true,
                    "conf_file": null,
                    "conf_dir": null,
                    "main_exists": false,
                    "files": [],
                })),
            ));
        };
        let mut files: Vec<Value> = Vec::new();
        let mut seen: std::collections::HashSet<String> = Default::default();
        let size = std::fs::metadata(&main).map(|m| m.len()).unwrap_or(0);
        let main_path = main.display().to_string();
        files.push(file_entry(&main, &root, true, size, main_exists));
        seen.insert(main_path);
        if root.is_dir() {
            collect_files(d, &root, &mut files);
            // 目录扫描可能再次命中主配置文件，去重
            files.retain(|f| {
                let p = f
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                seen.insert(p)
            });
        }
        Ok(Response::ok(
            "ok",
            Some(json!({
                "installed": true,
                "conf_file": main.display().to_string(),
                "conf_dir": root.display().to_string(),
                "main_exists": main_exists,
                "files": files,
            })),
        ))
    })
    .await
}

/// service_conf.read
pub async fn conf_read(svc: &str, path: String) -> Response {
    let svc = svc.to_string();
    run_blocking(move || {
        let Some(d) = supported(&svc) else {
            return Err(format!("不支持的服务类型: {svc}"));
        };
        let (_, _, Some((main, root, _))) = installed_info(d, &svc) else {
            return Err(format!("{} 未安装或未探测到配置，请先在应用商店安装", d.label));
        };
        let (canon, is_main) = validate_path(d, &main, &root, &path)?;
        if !canon.exists() {
            return Ok(Response::ok(
                "ok",
                Some(json!({
                    "path": canon.display().to_string(),
                    "is_main": is_main,
                    "content": "",
                    "size": 0,
                    "mtime": 0,
                    "missing": true,
                })),
            ));
        }
        let meta = std::fs::metadata(&canon).map_err(|e| format!("读取文件失败: {e}"))?;
        if meta.len() > MAX_FILE_BYTES {
            return Err("文件过大，不支持在线编辑".to_string());
        }
        let content = std::fs::read_to_string(&canon).map_err(|e| format!("读取文件失败: {e}"))?;
        Ok(Response::ok(
            "ok",
            Some(json!({
                "path": canon.display().to_string(),
                "is_main": is_main,
                "content": content,
                "size": meta.len(),
                "mtime": meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0),
            })),
        ))
    })
    .await
}

/// service_conf.save（json 格式先做合法性校验）。
pub async fn conf_save(svc: &str, path: String, content: String) -> Response {
    let svc = svc.to_string();
    run_blocking(move || {
        let Some(d) = supported(&svc) else {
            return Err(format!("不支持的服务类型: {svc}"));
        };
        if content.len() as u64 > MAX_FILE_BYTES {
            return Err("内容超过 2MB，请精简后重试".to_string());
        }
        let (_, _, Some((main, root, _))) = installed_info(d, &svc) else {
            return Err(format!("{} 未安装或未探测到配置", d.label));
        };
        let (canon, is_main) = validate_path(d, &main, &root, &path)?;
        if d.format == ConfFormat::Json && !content.trim().is_empty() {
            serde_json::from_str::<Value>(&content)
                .map_err(|e| format!("JSON 语法错误，未保存：{e}"))?;
        }
        if !is_main {
            // 编辑非主文件
            if !canon.exists() {
                return Err("文件不存在".to_string());
            }
        }
        let backup = backup_file(&svc, &canon)?;
        if let Some(parent) = canon.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
        }
        std::fs::write(&canon, &content).map_err(|e| format!("写入失败: {e}"))?;
        Ok(Response::ok(
            "ok",
            Some(json!({
                "path": canon.display().to_string(),
                "backup": backup.display().to_string(),
                "saved": true,
                "reason": "配置已保存，重启或重载服务后生效".to_string(),
            })),
        ))
    })
    .await
}

/// service_conf.keys：关键项表单定义 + 当前值。
pub async fn keys_get(svc: &str) -> Response {
    let svc = svc.to_string();
    run_blocking(move || {
        let Some(d) = supported(&svc) else {
            return Err(format!("不支持的服务类型: {svc}"));
        };
        let (bin, unit, main) = installed_info(d, &svc);
        if bin.is_none() && unit.is_none() {
            return Ok(Response::ok(
                "ok",
                Some(json!({
                    "installed": false,
                    "fields": [],
                    "values": {},
                })),
            ));
        }
        let fields: Vec<Value> = d
            .fields
            .iter()
            .map(|f| {
                let kind = match f.kind {
                    FieldKind::Text => "text",
                    FieldKind::Number => "number",
                    FieldKind::Select => "select",
                    FieldKind::Bool => "bool",
                };
                json!({
                    "key": f.key,
                    "label": f.label,
                    "kind": kind,
                    "help": f.help,
                    "section": f.section,
                    "options": f.options,
                })
            })
            .collect();
        let (values, main_path, main_exists) = match &main {
            Some((m, _, exists)) => {
                let content = std::fs::read_to_string(m).unwrap_or_default();
                let values = match d.format {
                    ConfFormat::Ini => ini_read_values(&content, d.ini_comment, &svc, d.fields),
                    ConfFormat::Json => json_read_values(&content, d.fields),
                };
                (values, Some(m.display().to_string()), *exists)
            }
            None => (Default::default(), None, false),
        };
        Ok(Response::ok(
            "ok",
            Some(json!({
                "installed": true,
                "service": svc,
                "label": d.label,
                "format": if d.format == ConfFormat::Json { "json" } else { "ini" },
                "main": main_path,
                "main_exists": main_exists,
                "fields": fields,
                "values": values,
            })),
        ))
    })
    .await
}

/// service_conf.keys_save：把关键项写回主配置文件（ini 托管块 / json 键合并）。
pub async fn keys_save(svc: &str, keys: std::collections::BTreeMap<String, String>) -> Response {
    let svc = svc.to_string();
    run_blocking(move || {
        let Some(d) = supported(&svc) else {
            return Err(format!("不支持的服务类型: {svc}"));
        };
        let (_, _, Some((main, _, _))) = installed_info(d, &svc) else {
            return Err(format!("{} 未安装或未探测到配置", d.label));
        };
        let old = if main.exists() {
            std::fs::read_to_string(&main).unwrap_or_default()
        } else {
            String::new()
        };
        let patched = match d.format {
            ConfFormat::Ini => {
                // 仅接收注册过的 key，且非空才写入（空 = 从托管区移除该键）
                let cleaned = strip_managed_block(&old, d.ini_comment, &svc);
                // 托管块内部为行式 key = value
                let entries: Vec<(&str, String)> = d
                    .fields
                    .iter()
                    .filter_map(|f| {
                        let v = keys.get(f.key)?.trim().to_string();
                        if v.is_empty() {
                            return None;
                        }
                        Some((f.key, v))
                    })
                    .collect();
                if entries.is_empty() {
                    cleaned
                } else {
                    let block = block_lines(d.ini_comment, &svc, &entries);
                    insert_block(&cleaned, d.fields.first().and_then(|f| f.section), &block)
                }
            }
            ConfFormat::Json => {
                let mut obj: Value = if old.trim().is_empty() {
                    json!({})
                } else {
                    serde_json::from_str(&old).unwrap_or_else(|_| json!({}))
                };
                for f in d.fields {
                    if let Some(raw) = keys.get(f.key) {
                        let raw = raw.trim();
                        if raw.is_empty() {
                            // 留空 = 保持原文件中的既有键不动（不写入托管值）
                            continue;
                        }
                        let v = match f.kind {
                            FieldKind::Bool => {
                                Value::Bool(raw.eq_ignore_ascii_case("true") || raw == "1")
                            }
                            FieldKind::Number => raw
                                .parse::<i64>()
                                .map(Value::from)
                                .unwrap_or_else(|_| Value::String(raw.to_string())),
                            _ => Value::String(raw.to_string()),
                        };
                        set_json_path(&mut obj, f.jpath, v);
                    }
                }
                serde_json::to_string_pretty(&obj).map_err(|e| format!("序列化 JSON 失败: {e}"))?
            }
        };
        let backup = backup_file(&svc, &main)?;
        if let Some(parent) = main.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
        }
        std::fs::write(&main, &patched).map_err(|e| format!("写入失败: {e}"))?;
        Ok(Response::ok(
            "ok",
            Some(json!({
                "path": main.display().to_string(),
                "backup": backup.display().to_string(),
                "saved": true,
                "reason": "关键配置已保存，重启或重载服务后生效".to_string(),
            })),
        ))
    })
    .await
}

/// service_conf.control：start / stop / restart / reload。
pub async fn control(svc: &str, action: &str) -> Response {
    let svc = svc.to_string();
    let action = action.to_string();
    run_blocking(move || {
        if !ALLOWED_ACTIONS.contains(&action.as_str()) {
            return Err(format!(
                "不支持的操作: {action}（仅支持 {}）",
                ALLOWED_ACTIONS.join(" / ")
            ));
        }
        let Some(d) = supported(&svc) else {
            return Err(format!("不支持的服务类型: {svc}"));
        };
        let bin = find_bin(d);
        // 优先已注册/存在的 unit（实例 svc 走 php-fpm-<ver>）；其次用二进制的 systemd 名做一次尝试
        let unit = effective_unit(d, &svc).or_else(|| {
            bin.as_ref()
                .and_then(|b| b.file_stem())
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        });
        if unit.is_none() {
            return Err(format!("{} 未安装或未检测到 systemd 服务", d.label));
        }
        let unit = unit.unwrap();
        let method = if systemd_has(&unit) {
            let o = root_cmd("systemctl")
                .args([action.as_str(), unit.as_str()])
                .output()
                .map_err(|e| format!("执行 systemctl {} {} 失败: {e}", action, unit))?;
            if !o.status.success() {
                return Err(format!(
                    "systemctl {} {} 失败：{}",
                    action,
                    unit,
                    String::from_utf8_lossy(&o.stderr)
                        .trim()
                        .chars()
                        .take(2000)
                        .collect::<String>()
                ));
            }
            if action == "restart" {
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            "systemd"
        } else {
            return Err(format!("未找到 systemd 服务 {}，请确认服务安装方式", unit));
        };
        if action == "start" || action == "restart" {
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
        let running = service_running(d, &svc, bin.as_deref());
        let state = if running { "running" } else { "stopped" };
        Ok(Response::ok(
            "ok",
            Some(json!({
                "action": action,
                "unit": unit,
                "method": method,
                "state": state,
            })),
        ))
    })
    .await
}

// ── PHP 多版本实例：列表 + 全局默认访问 ─────────────────────────

/// 全局命令注册目录（install.sh 把实例注册到此处供所有用户直接使用）。
const GLOBAL_BIN_DIR: &str = "/usr/local/bin";
/// 全局默认访问注册的命令名（与应用商店 php install.sh 一致）。
const GLOBAL_LINK_NAMES: &[&str] = &["php", "php-cgi", "pear", "pecl"];

fn global_bin_link(name: &str) -> PathBuf {
    PathBuf::from(GLOBAL_BIN_DIR).join(name)
}

/// 宽松等价比较：优先 canonicalize，失败回退字符串相等（失效链接场景）。
fn paths_equal(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => a == b,
    }
}

/// 当前 /usr/local/bin/php 全局默认指向的目标（无注册链接则 None）。
fn global_default_bin() -> Option<PathBuf> {
    std::fs::read_link(global_bin_link("php")).ok()
}

/// 单个 PHP 实例的概要（instances 列表项），字段与 status 对齐。
fn instance_summary(svc: &str) -> Result<Value, String> {
    let Some(d) = supported(svc) else {
        return Err(format!("不支持的服务类型: {svc}"));
    };
    let (bin, unit, main) = installed_info(d, svc);
    let installed = bin.is_some() || unit.is_some();
    let (conf_file, main_exists) = match &main {
        Some((m, _, exists)) => (Some(m.display().to_string()), *exists),
        None => (None, false),
    };
    let version = if installed {
        bin.as_deref()
            .map(|b| detect_version(d, b))
            .unwrap_or_default()
    } else {
        String::new()
    };
    let is_default = bin.as_deref().is_some_and(|b| {
        global_default_bin().is_some_and(|t| paths_equal(&t, b))
    });
    let dir = php_inst(svc).map(|i| i.dir.display().to_string());
    Ok(json!({
        "svc": svc,
        "instance": svc,
        "label": d.label,
        "version": version,
        "dir": dir,
        "installed": installed,
        "running": installed && service_running(d, svc, bin.as_deref()),
        "bin": bin.as_deref().map(|b| b.display().to_string()),
        "unit": unit,
        "systemd": unit.is_some(),
        "conf_file": conf_file,
        "main_exists": main_exists,
        "is_default": is_default,
    }))
}

/// service_conf.instances：列出某服务的全部已安装版本实例。
/// 目前支持 php（应用商店 php-<ver> 目录实例）；无目录实例时回退类型级
/// php（系统包），保证既有「服务配置 → PHP」页能力不回归。
pub async fn instances(svc: &str) -> Response {
    let svc = svc.to_string();
    run_blocking(move || {
        if svc != "php" {
            return Err("实例列表目前仅支持 php 服务".to_string());
        }
        let mut list: Vec<Value> = Vec::new();
        if let Ok(rd) = std::fs::read_dir(super::install_root()) {
            let mut dirs: Vec<PathBuf> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.is_dir()
                        && p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n.starts_with("php-"))
                })
                .collect();
            dirs.sort();
            for dir in dirs {
                let Some(digits) = dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.trim_start_matches("php-").to_string())
                else {
                    continue;
                };
                if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                let s = format!("php{digits}");
                if let Ok(v) = instance_summary(&s)
                    && v.get("installed").and_then(|x| x.as_bool()).unwrap_or(false)
                {
                    list.push(v);
                }
            }
        }
        // 未发现目录实例（非应用商店安装）时，兼容系统包 PHP
        if list.is_empty()
            && let Ok(v) = instance_summary("php")
            && v.get("installed").and_then(|x| x.as_bool()).unwrap_or(false)
        {
            list.push(v);
        }
        Ok(Response::ok("ok", Some(json!({ "instances": list }))))
    })
    .await
}

/// service_conf.default：设置 / 取消某 PHP 实例的「全局默认访问」。
///
/// - enable=true：把该实例注册到 `/usr/local/bin`（php / php-cgi / pear /
///   pecl），系统所有用户执行 `php` 默认使用本版本；`/usr/bin/php` 沿用
///   install.sh 语义仅在不存在时补链，避免抢占 Debian alternatives。
/// - enable=false：仅当 `/usr/local/bin/php` 当前指向本实例时移除注册
///   （已被其它实例接管则拒绝取消，避免误删别家链接）。
pub async fn set_default(svc: &str, enable: bool) -> Response {
    let svc = svc.to_string();
    run_blocking(move || {
        let Some(inst) = php_inst(&svc) else {
            return Err(format!("未找到 PHP 实例 {svc}（安装目录不存在）"));
        };
        let bin_dir = inst.dir.join("bin");
        if enable {
            let mut registered: Vec<String> = Vec::new();
            for name in GLOBAL_LINK_NAMES {
                let src = bin_dir.join(name);
                if !src.is_file() {
                    continue;
                }
                let link = global_bin_link(name);
                let _ = std::fs::remove_file(&link);
                std::os::unix::fs::symlink(&src, &link)
                    .map_err(|e| format!("注册 /usr/local/bin/{name} 失败: {e}"))?;
                registered.push(name.to_string());
            }
            if registered.is_empty() {
                return Err(format!("实例 {svc} 的 bin 目录为空，无法注册全局命令"));
            }
            let ub = PathBuf::from("/usr/bin/php");
            if !ub.exists() && bin_dir.join("php").is_file() {
                let _ = std::os::unix::fs::symlink(bin_dir.join("php"), &ub);
            }
            Ok(Response::ok(
                "ok",
                Some(json!({
                    "enabled": true,
                    "service": svc,
                    "registered": registered,
                })),
            ))
        } else {
            let mine = bin_dir.join("php");
            let cur = std::fs::read_link(global_bin_link("php")).unwrap_or_default();
            if !paths_equal(&cur, &mine) {
                return Err(
                    "当前 /usr/local/bin/php 不指向该实例（未注册或被其它实例接管），无需取消"
                        .to_string(),
                );
            }
            let mut removed: Vec<String> = Vec::new();
            for name in GLOBAL_LINK_NAMES {
                let src = bin_dir.join(name);
                let link = global_bin_link(name);
                let owned = std::fs::read_link(&link).unwrap_or_default();
                if paths_equal(&owned, &src) {
                    let _ = std::fs::remove_file(&link);
                    removed.push(name.to_string());
                }
            }
            Ok(Response::ok(
                "ok",
                Some(json!({
                    "enabled": false,
                    "service": svc,
                    "removed": removed,
                })),
            ))
        }
    })
    .await
}
