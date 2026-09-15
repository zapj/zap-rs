//! 服务器运行环境探测（root 执行，只读）。
//!
//! 探测 OS / 主机名 / Web 服务器（nginx|openresty）/ PHP（含 FPM socket）/
//! 数据库实例 / 常用工具链，供面板「运行环境」页展示与全局默认配置使用。
//! 单项探测失败以空串 / 空数组表示，不影响整体成功返回。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use zap_proto::Response;

use super::root_cmd;
use super::site;

pub async fn detect() -> Response {
    tokio::task::spawn_blocking(move || -> Result<Response, String> { Ok(detect_inner()) })
        .await
        .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
        .unwrap_or_else(|e| Response::err(-1, e))
}

fn detect_inner() -> Response {
    let (os_id, os_name, os_ver) = os_release();
    let data = json!({
        "os": {
            "id": os_id,
            "name": os_name,
            "version": os_ver,
            "arch": std::env::consts::ARCH,
            "kernel": kernel_version(),
        },
        "hostname": hostname_detect(),
        "webserver": detect_webserver(),
        "php": detect_php(),
        "databases": detect_databases(),
        "tools": detect_tools(),
        "network": detect_network(),
    });
    Response::ok("服务器运行环境探测完成", Some(data))
}

// ── 通用小工具 ────────────────────────────────────────────────

/// 运行命令并返回首个非空行（优先 stdout，其次 stderr），失败返回 None。
fn probe_first_line(program: &str, args: &[&str]) -> Option<String> {
    let o = root_cmd(program).args(args).output().ok()?;
    let text = if !o.stdout.is_empty() {
        String::from_utf8_lossy(&o.stdout).into_owned()
    } else {
        String::from_utf8_lossy(&o.stderr).into_owned()
    };
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    Some(line.chars().take(180).collect())
}

/// 进程是否在运行（精确匹配进程名）。
fn proc_running(name: &str) -> bool {
    root_cmd("pgrep")
        .args(["-x", name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 在 marker 之后的第一个空白分隔 token，如失败返回 None。
fn token_after(line: &str, marker: &str) -> Option<String> {
    let rest = line.split_once(marker)?.1;
    let tok = rest
        .split_whitespace()
        .next()?
        .trim_matches(['(', ')', ';', ',']);
    if tok.is_empty() {
        None
    } else {
        Some(tok.to_string())
    }
}

// ── OS / 主机 ────────────────────────────────────────────────

fn os_release() -> (String, String, String) {
    let mut id = "linux".to_string();
    let mut name = "Linux".to_string();
    let mut ver = String::new();
    if let Ok(text) = std::fs::read_to_string("/etc/os-release") {
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let v = v.trim().trim_matches('"').to_string();
            match k {
                "ID" => id = v,
                "NAME" => name = v,
                "VERSION_ID" => ver = v,
                _ => {}
            }
        }
    }
    (id, name, ver)
}

fn kernel_version() -> String {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| probe_first_line("uname", &["-r"]))
        .unwrap_or_default()
}

fn hostname_detect() -> String {
    let from_file = std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .ok()
        .filter(|s| !s.is_empty());
    from_file
        .or_else(|| probe_first_line("hostname", &[]))
        .unwrap_or_default()
}

// ── Web 服务器（nginx / openresty）────────────────────────────

fn detect_webserver() -> Value {
    if let Some(conf) = site::find_nginx_conf_file() {
        let conf_s = conf.to_string_lossy().into_owned();
        let bin = site::nginx_bin(&conf);
        let bin_s = bin.to_string_lossy().into_owned();
        let raw = probe_first_line(&bin_s, &["-v"]).unwrap_or_default();
        let flavor = if raw.contains("openresty")
            || conf_s.contains("openresty")
            || bin_s.contains("openresty")
        {
            "openresty"
        } else {
            "nginx"
        };
        let sites_dir = site::vhosts_dir(&conf)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        return json!({
            "flavor": flavor,
            "version": nginx_version(&raw),
            "binary": bin_s,
            "conf": conf_s,
            "sites_dir": sites_dir,
            "running": site::nginx_running(),
        });
    }
    // 系统自装 nginx（apt/yum），未纳入 data/apps
    if let Some(raw) = probe_first_line("nginx", &["-v"]) {
        let conf = if Path::new("/etc/nginx/nginx.conf").is_file() {
            "/etc/nginx/nginx.conf"
        } else {
            ""
        };
        let sites_dir = if Path::new("/etc/nginx/sites-enabled").is_dir() {
            "/etc/nginx/sites-enabled"
        } else if Path::new("/etc/nginx/conf.d").is_dir() {
            "/etc/nginx/conf.d"
        } else {
            ""
        };
        return json!({
            "flavor": "nginx",
            "version": nginx_version(&raw),
            "binary": "nginx",
            "conf": conf,
            "sites_dir": sites_dir,
            "running": site::nginx_running(),
        });
    }
    json!({ "flavor": "none", "version": "", "binary": "", "conf": "", "sites_dir": "", "running": false })
}

/// 从 `nginx version: nginx/1.24.0` / `nginx version: openresty/1.25.3.2` 中取版本号。
fn nginx_version(raw: &str) -> String {
    let Some(idx) = raw.rfind('/') else {
        return String::new();
    };
    raw[idx + 1..]
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches('.')
        .trim_end_matches(';')
        .to_string()
}

// ── PHP ──────────────────────────────────────────────────────

fn detect_php() -> Value {
    // key: 短版本号（如 8.3）
    let mut instances: BTreeMap<String, Value> = BTreeMap::new();

    // 1) AppStore 安装：/usr/local/apps/php*
    if let Ok(rd) = std::fs::read_dir("/usr/local/apps") {
        let mut dirs: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_dir()
                    && p.file_name()
                        .map(|n| n.to_string_lossy().starts_with("php"))
                        .unwrap_or(false)
            })
            .collect();
        dirs.sort();
        for d in dirs {
            let bin = d.join("bin/php");
            if bin.is_file() {
                push_php_bin(&mut instances, &bin);
            }
        }
    }

    // 2) 系统 PHP：/usr/bin/phpX*、/usr/local/bin/phpX*
    for base in [PathBuf::from("/usr/bin"), PathBuf::from("/usr/local/bin")] {
        if let Ok(rd) = std::fs::read_dir(&base) {
            for e in rd.flatten() {
                let p = e.path();
                if !p.is_file() {
                    continue;
                }
                let Some(name) = p.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                    continue;
                };
                let rest = name.strip_prefix("php").unwrap_or("");
                if rest.is_empty() || !rest.chars().next().unwrap().is_ascii_digit() {
                    continue;
                }
                push_php_bin(&mut instances, &p);
            }
        }
    }

    // 3) FPM socket：为实例补 socket / running；未匹配 socket 生成独立条目
    for sock in fpm_sockets() {
        let running = sock.exists();
        let Some(raw_tok) = socket_version_token(&sock) else {
            continue;
        };
        let v2 = normalize_version_token(&raw_tok);
        if v2.is_empty() {
            continue;
        }
        if let Some(ins) = instances.get_mut(&v2) {
            if running {
                if ins["socket"].as_str().unwrap_or("").is_empty() {
                    ins["socket"] = json!(sock.to_string_lossy());
                }
                ins["running"] = json!(true);
            }
            continue;
        }
        let entry = instances.entry(v2.clone()).or_insert_with(
            || json!({ "version": v2.clone(), "binary": "", "socket": "", "running": false }),
        );
        if running {
            entry["socket"] = json!(sock.to_string_lossy());
            entry["running"] = json!(true);
        }
    }

    // 4) 默认 PHP（PATH 上的 php；无则取最高版本）
    let mut default_v2 = String::new();
    if let Some(line) = probe_first_line("php", &["-v"])
        && let Some(ver) = php_version(&line)
    {
        default_v2 = short_version(&ver);
    }
    if default_v2.is_empty() {
        default_v2 = instances.keys().next_back().cloned().unwrap_or_default();
    }

    let mut list: Vec<Value> = Vec::with_capacity(instances.len());
    for (v2, mut ins) in instances {
        ins["default"] = json!(v2 == default_v2);
        list.push(ins);
    }

    json!({ "default": default_v2, "instances": list })
}

fn push_php_bin(map: &mut BTreeMap<String, Value>, bin: &Path) {
    let bstr = bin.to_string_lossy().into_owned();
    let Some(line) = probe_first_line(&bstr, &["-v"]) else {
        return;
    };
    let Some(ver) = php_version(&line) else {
        return;
    };
    let v2 = short_version(&ver);
    if map.contains_key(&v2) {
        return;
    }
    map.insert(
        v2.clone(),
        json!({
            "version": ver,
            "binary": bstr,
            "socket": "",
            "running": false,
            "default": false,
        }),
    );
}

/// 从 `PHP 8.3.7 (cli)` 第一行中提取完整版本号（8.3.7）。
fn php_version(line: &str) -> Option<String> {
    let tok = line.trim().strip_prefix("PHP")?.split_whitespace().next()?;
    if tok.starts_with(|c: char| c.is_ascii_digit()) {
        Some(tok.to_string())
    } else {
        None
    }
}

fn short_version(ver: &str) -> String {
    ver.split('.').take(2).collect::<Vec<_>>().join(".")
}

/// 常见 FPM unix socket 位置扫描（系统 pool 与单实例均可）。
fn fpm_sockets() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in [
        PathBuf::from("/var/run"),
        PathBuf::from("/run"),
        PathBuf::from("/var/run/php-fpm"),
    ] {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let Some(name) = p.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                continue;
            };
            if name.starts_with("php-fpm") && name.ends_with(".sock") {
                out.push(p);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// 从 socket 文件名中取版本 token：`php-fpm-8.3.sock` → `8.3`；`php-fpm83.sock` → `83`。
fn socket_version_token(sock: &Path) -> Option<String> {
    let name = sock.file_name()?.to_string_lossy();
    let name = name.strip_suffix(".sock")?;
    let tok = name.strip_prefix("php-fpm")?.trim_start_matches(['-', '_']);
    if tok.is_empty() {
        None
    } else {
        Some(tok.to_string())
    }
}

/// 版本 token 规范化：`83` → `8.3`；含点的原样保留。
fn normalize_version_token(t: &str) -> String {
    let t = t.trim().trim_matches(['-', '_', '.']);
    if t.is_empty() {
        return String::new();
    }
    if t.contains('.') {
        return t.to_string();
    }
    let is_digit2 = t.len() == 2 && t.bytes().all(|b| b.is_ascii_digit());
    if is_digit2 {
        return format!("{}.{}", &t[..1], &t[1..]);
    }
    t.to_string()
}

// ── 数据库 ───────────────────────────────────────────────────

fn detect_databases() -> Value {
    let mut out = Vec::new();
    for (name, bins, prefixes) in [
        ("mysql", &["mysqld"][..], &["mysql"][..]),
        ("mariadb", &["mariadbd", "mysqld"][..], &["mariadb"][..]),
        ("postgresql", &["postgres"][..], &["postgres", "pgsql"][..]),
        ("redis", &["redis-server"][..], &["redis"][..]),
        ("mongodb", &["mongod"][..], &["mongo"][..]),
    ] {
        if let Some((binary, line)) = probe_database(bins, prefixes) {
            let proc = Path::new(&binary)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| bins[0].to_string());
            out.push(json!({
                "name": name,
                "version": db_version(name, &line),
                "binary": binary,
                "running": proc_running(&proc),
            }));
        }
    }
    json!(out)
}

/// 探测数据库守护进程：安装根（AppStore，含版本目录）优先，其次 PATH。
/// 返回 (二进制路径, 版本首行)。
fn probe_database(bins: &[&str], prefixes: &[&str]) -> Option<(String, String)> {
    for bin in bins {
        for cand in db_bin_candidates(bin, prefixes) {
            let path = cand.to_string_lossy().into_owned();
            if let Some(line) = probe_first_line(&path, &["--version"]) {
                return Some((path, line));
            }
        }
    }
    // 系统包管理器安装（在 PATH 上）
    for bin in bins {
        if let Some(line) = probe_first_line(bin, &["--version"]) {
            return Some((bin.to_string(), line));
        }
    }
    None
}

/// 安装根（默认 /usr/local/apps，`ZAP_APPS_DIR` 可覆盖）下查找守护进程：
/// 兼容 `<root>/mysql-8.4` 与 `<root>/<分类>/mysql-8.4` 两种布局，
/// 多个版本时优先版本号高者。
fn db_bin_candidates(bin: &str, prefixes: &[&str]) -> Vec<PathBuf> {
    let root = super::install_root();
    let matched = |name: &str| prefixes.iter().any(|pre| name.starts_with(pre));

    let Ok(top) = std::fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = Vec::new();
    for e in top.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        if matched(&e.file_name().to_string_lossy()) {
            dirs.push(p.clone());
        }
        // 分类二级目录（database / server / ...）
        if let Ok(sub) = std::fs::read_dir(&p) {
            for e2 in sub.flatten() {
                let q = e2.path();
                if q.is_dir() && matched(&e2.file_name().to_string_lossy()) {
                    dirs.push(q);
                }
            }
        }
    }

    dirs.sort_by_key(|d| {
        d.file_name()
            .map(|n| dir_version_segments(&n.to_string_lossy()))
            .unwrap_or_default()
    });
    dirs.reverse();
    dirs.dedup();

    let mut out = Vec::new();
    for d in dirs {
        for sub in ["bin", "sbin"] {
            let c = d.join(sub).join(bin);
            if c.is_file() {
                out.push(c);
                break;
            }
        }
    }
    out
}

/// 提取目录名中的数字段（`mysql-8.4` → `[8, 4]`），用于版本排序。
fn dir_version_segments(name: &str) -> Vec<u64> {
    name.split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<u64>().ok())
        .collect()
}

fn db_version(name: &str, line: &str) -> String {
    let raw = match name {
        "mysql" | "mariadb" => token_after(line, "Ver ").unwrap_or_default(),
        "postgresql" => token_after(line, "PostgreSQL) ").unwrap_or_default(),
        "redis" => token_after(line, "v=").unwrap_or_default(),
        "mongodb" => token_after(line, "version v").unwrap_or_default(),
        _ => String::new(),
    };
    // 去掉发行版/插件后缀：8.0.39-0ubuntu... → 8.0.39；11.4.4-MariaDB → 11.4.4
    let head = raw.split('-').next().unwrap_or(&raw).to_string();
    let head = head.trim_end_matches(['(', ')', ';', ',']).to_string();
    if head.is_empty() {
        line.chars().take(80).collect()
    } else {
        head
    }
}

// ── 常用工具链 ───────────────────────────────────────────────

fn detect_tools() -> Value {
    let mut out = Vec::new();
    for (name, args) in [
        ("git", &["--version"][..]),
        ("node", &["--version"][..]),
        ("docker", &["--version"][..]),
        ("python3", &["--version"][..]),
        ("composer", &["--version"][..]),
        ("php", &["--version"][..]),
    ] {
        if let Some(line) = probe_first_line(name, args) {
            out.push(json!({ "name": name, "version": line }));
        }
    }
    json!(out)
}

// ── 网络接口 / IP ────────────────────────────────────────────

/// 需要排除的虚拟/容器接口前缀（面板建站不会绑定这些地址）。
const SKIP_IFACE_PREFIXES: [&str; 8] = [
    "lo", "docker", "br-", "veth", "virbr", "cni", "flannel", "tunl",
];

fn skip_iface(name: &str) -> bool {
    name == "lo" || SKIP_IFACE_PREFIXES.iter().any(|p| name.starts_with(p))
}

/// 解析 `ip -o -4 addr show` 输出，得到「接口 → IPv4 列表」。
///
/// 例：`2: eth0    inet 10.0.0.5/24 brd ...` → `("eth0", "10.0.0.5")`。
fn parse_ipv4_lines(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        // 形如 `2: eth0    inet 10.0.0.5/24 ...`
        let Some(idx) = it.next() else { continue };
        let Some(iface) = it.next() else { continue };
        let iface = iface.trim_end_matches(':').to_string();
        if !idx.ends_with(':') || skip_iface(&iface) {
            continue;
        }
        // 找到 `inet` 后取地址段
        let mut it = line.split_whitespace().skip_while(|t| *t != "inet");
        let _ = it.next();
        let Some(addr) = it.next() else { continue };
        let ip = addr.split('/').next().unwrap_or("").to_string();
        if ip.is_empty() {
            continue;
        }
        out.push((iface, ip));
    }
    out
}

/// 解析 `ip -o -6 addr show` 输出，得到「接口 → IPv6 列表」（去掉 scope link）。
fn parse_ipv6_lines(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        if !line.contains(" inet6 ") {
            continue;
        }
        let mut it = line.split_whitespace();
        let Some(idx) = it.next() else { continue };
        let Some(iface) = it.next() else { continue };
        let iface = iface.trim_end_matches(':').to_string();
        if !idx.ends_with(':') || skip_iface(&iface) {
            continue;
        }
        let mut it = line.split_whitespace().skip_while(|t| *t != "inet6");
        let _ = it.next();
        let Some(addr) = it.next() else { continue };
        let ip = addr.split('/').next().unwrap_or("").to_string();
        // 链路本地地址（fe80::）不作为对外服务地址候选
        if ip.is_empty() || ip.to_ascii_lowercase().starts_with("fe80") {
            continue;
        }
        out.push((iface, ip));
    }
    out
}

fn run_ip(family: &str) -> String {
    root_cmd("ip")
        .args(["-o", family, "addr", "show"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// 网卡与地址探测：供面板「基础设置 → 默认 IPv4/IPv6/网络设备」下拉选择。
///
/// 结构：`{ interfaces: [{name, mac, state, ipv4[], ipv6[]}], default_ipv4, default_ipv6 }`。
/// `default_*` 取首个 UP 且带地址的网卡的对应地址（`ip route get` 失败时兜底）。
fn detect_network() -> Value {
    use std::collections::BTreeMap;

    let v4 = parse_ipv4_lines(&run_ip("-4"));
    let v6 = parse_ipv6_lines(&run_ip("-6"));

    let mut order: Vec<String> = Vec::new();
    let mut map: BTreeMap<String, Value> = BTreeMap::new();
    for (iface, ip) in v4.iter() {
        if !order.contains(iface) {
            order.push(iface.clone());
        }
        let e = map.entry(iface.clone()).or_insert_with(|| {
            json!({
                "name": iface,
                "mac": iface_mac(iface),
                "state": iface_state(iface),
                "ipv4": Vec::<String>::new(),
                "ipv6": Vec::<String>::new(),
            })
        });
        if let Some(arr) = e.get_mut("ipv4").and_then(|v| v.as_array_mut())
            && !arr.iter().any(|x| x == ip)
        {
            arr.push(json!(ip));
        }
    }
    for (iface, ip) in v6.iter() {
        let e = map.entry(iface.clone()).or_insert_with(|| {
            json!({
                "name": iface,
                "mac": iface_mac(iface),
                "state": iface_state(iface),
                "ipv4": Vec::<String>::new(),
                "ipv6": Vec::<String>::new(),
            })
        });
        if let Some(arr) = e.get_mut("ipv6").and_then(|v| v.as_array_mut())
            && !arr.iter().any(|x| x == ip)
        {
            arr.push(json!(ip));
        }
    }

    let interfaces: Vec<Value> = map.values().cloned().collect();
    // 默认出口地址：`ip route get 1.1.1.1` 的 src；失败时取首个网卡的首个地址
    let default_ipv4 = default_route_ip("-4").or_else(|| v4.first().map(|(_, ip)| ip.clone()));
    let default_ipv6 = default_route_ip("-6").or_else(|| v6.first().map(|(_, ip)| ip.clone()));

    json!({
        "interfaces": interfaces,
        "ipv4_all": v4.iter().map(|(_, ip)| ip.clone()).collect::<Vec<_>>(),
        "ipv6_all": v6.iter().map(|(_, ip)| ip.clone()).collect::<Vec<_>>(),
        "default_ipv4": default_ipv4.unwrap_or_default(),
        "default_ipv6": default_ipv6.unwrap_or_default(),
    })
}

fn iface_mac(iface: &str) -> String {
    std::fs::read_to_string(format!("/sys/class/net/{iface}/address"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn iface_state(iface: &str) -> String {
    std::fs::read_to_string(format!("/sys/class/net/{iface}/operstate"))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

/// `ip -o route get` 输出里的 `src <ip>`。
fn default_route_ip(family: &str) -> Option<String> {
    let out = root_cmd("ip")
        .args(["-o", family, "route", "get", "1.1.1.1"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut it = text.split_whitespace().skip_while(|t| *t != "src");
    let _ = it.next();
    it.next().map(|s| s.to_string())
}

// ── 单测（纯函数部分）────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_nginx_version_lines() {
        assert_eq!(nginx_version("nginx version: nginx/1.24.0"), "1.24.0");
        assert_eq!(
            nginx_version("nginx version: openresty/1.25.3.2"),
            "1.25.3.2"
        );
        assert_eq!(nginx_version("nginx: [emerg] unknown directive"), "");
    }

    #[test]
    fn parse_php_version_lines() {
        assert_eq!(
            php_version("PHP 8.3.7 (cli) (built: Jun 27 2024)").unwrap(),
            "8.3.7"
        );
        assert_eq!(php_version("PHP 7.4.33").unwrap(), "7.4.33");
        assert!(php_version("Usage: php [options]").is_none());
        assert_eq!(short_version("8.3.7"), "8.3");
        assert_eq!(short_version("7.4"), "7.4");
    }

    #[test]
    fn parse_ip_addr_lines() {
        let v4 = "2: eth0    inet 10.0.0.5/24 brd 10.0.0.255 scope global eth0\\       valid_lft forever preferred_lft forever\n\
                  3: eth1    inet 192.168.1.7/24 scope global eth1\n\
                  1: lo    inet 127.0.0.1/8 scope host lo\n";
        assert_eq!(
            parse_ipv4_lines(v4),
            vec![
                ("eth0".to_string(), "10.0.0.5".to_string()),
                ("eth1".to_string(), "192.168.1.7".to_string())
            ]
        );

        let v6 = "2: eth0    inet6 2408:4005:xxx::1/64 scope global \n\
                  2: eth0    inet6 fe80::1/64 scope link \n\
                  1: lo    inet6 ::1/128 scope host \n";
        assert_eq!(
            parse_ipv6_lines(v6),
            vec![("eth0".to_string(), "2408:4005:xxx::1".to_string())]
        );
    }

    #[test]
    fn socket_tokens_normalized() {
        assert_eq!(
            socket_version_token(Path::new("/var/run/php-fpm-8.3.sock")).unwrap(),
            "8.3"
        );
        assert_eq!(
            normalize_version_token(
                &socket_version_token(Path::new("/var/run/php-fpm-8.3.sock")).unwrap()
            ),
            "8.3"
        );
        assert_eq!(
            normalize_version_token(
                &socket_version_token(Path::new("/var/run/php-fpm83.sock")).unwrap()
            ),
            "8.3"
        );
        // 无版本 token 的 socket 文件名返回 None → 归一化空串（调用方跳过）
        assert!(socket_version_token(Path::new("/var/run/php-fpm.sock")).is_none());
        assert_eq!(normalize_version_token(""), "");
    }

    #[test]
    fn dir_version_ordering() {
        assert_eq!(dir_version_segments("mysql-8.4"), vec![8, 4]);
        assert_eq!(dir_version_segments("php-74"), vec![74]);
        assert!(dir_version_segments("nginx").is_empty());
        // 数值比较：10.0 高于 9.0（字符串排序会判反）
        assert!(dir_version_segments("mysql-10.0") > dir_version_segments("mysql-9.0"));
    }

    #[test]
    fn db_version_parsing() {
        assert_eq!(
            db_version(
                "mysql",
                "mysqld  Ver 8.4.0 for Linux on x86_64 (MySQL Community Server - GPL)"
            ),
            "8.4.0"
        );
        assert_eq!(
            db_version(
                "mariadb",
                "mariadbd  Ver 10.11.6-MariaDB for Linux on x86_64"
            ),
            "10.11.6"
        );
        assert_eq!(
            db_version("postgresql", "postgres (PostgreSQL) 16.2"),
            "16.2"
        );
        assert_eq!(
            db_version("redis", "Redis server v=7.2.4 sha=00000000:0"),
            "7.2.4"
        );
        assert_eq!(db_version("mongodb", "db version v7.0.5"), "7.0.5");
    }
}
