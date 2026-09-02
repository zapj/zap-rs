//! SSH 密钥管理 verb。
//!
//! 密钥存储在 `/etc/zap/ssh`（`zap_proto::SSH_KEY_DIR`），由 zapexec（root）写入、
//! zapd（zapadm）读取。原先依赖 `/root/.ssh`，zapd 以 zapadm 身份无法访问。
//! 私钥/authorized_keys 权限统一为 `root:zapadm` 0640，便于 zapd 读取用于 SSH 连接。

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use zap_proto::{Response, SSH_KEY_DIR};

fn ssh_dir() -> PathBuf {
    PathBuf::from(SSH_KEY_DIR)
}

/// 设权限 + 属主（root:zapadm），静默失败（开发态非 root）。
fn set_owned_mode(path: &Path, gid: u32, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    if let Ok(c) = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()) {
        unsafe {
            libc::chown(c.as_ptr(), 0, gid);
        }
    }
}

fn ensure_ssh_dir(gid: u32) -> Result<(), String> {
    let dir = ssh_dir();
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("无法创建密钥目录: {e}"))?;
    }
    set_owned_mode(&dir, gid, 0o750);
    Ok(())
}

/// 校验密钥名，防路径穿越。
fn valid_name(name: &str) -> bool {
    !name.is_empty() && !name.contains('/') && !name.contains("..")
}

fn fingerprint_of(pub_path: &Path) -> String {
    std::process::Command::new("ssh-keygen")
        .args(["-lf", &pub_path.to_string_lossy()])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let out = String::from_utf8_lossy(&o.stdout);
                out.split_whitespace().nth(1).map(|s| s.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn is_authorized(pub_line: &str) -> bool {
    let auth_path = ssh_dir().join("authorized_keys");
    if !auth_path.exists() {
        return false;
    }
    std::fs::read_to_string(&auth_path)
        .map(|ak| ak.lines().any(|l| l.trim() == pub_line))
        .unwrap_or(false)
}

fn parse_pub_file(path: &Path) -> Option<Value> {
    let content = std::fs::read_to_string(path).ok()?;
    let parts: Vec<&str> = content.trim().splitn(3, ' ').collect();
    if parts.len() < 2 {
        return None;
    }
    let key_type = parts[0].to_string();
    let comment = parts.get(2).unwrap_or(&"").to_string();
    let name = path
        .file_stem()
        .map(|n| {
            let s = n.to_string_lossy();
            s.strip_suffix(".pub").unwrap_or(&s).to_string()
        })
        .unwrap_or_else(|| "unknown".to_string());

    let fingerprint = fingerprint_of(path);
    let bits = fingerprint
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    let created_at = path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| {
            chrono::DateTime::<chrono::Utc>::from(t)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_default();

    Some(json!({
        "name": name,
        "key_type": key_type,
        "bits": bits,
        "fingerprint": fingerprint,
        "comment": comment,
        "public_key": content.trim(),
        "authorized": is_authorized(content.trim()),
        "created_at": created_at,
    }))
}

fn remove_from_authorized_keys(pub_line: &str) {
    let auth_path = ssh_dir().join("authorized_keys");
    if !auth_path.exists() {
        return;
    }
    if let Ok(content) = std::fs::read_to_string(&auth_path) {
        let new_content: String = content
            .lines()
            .filter(|l| l.trim() != pub_line)
            .collect::<Vec<_>>()
            .join("\n");
        let new_content = new_content.trim().to_string() + "\n";
        std::fs::write(&auth_path, new_content).ok();
    }
}

// ── 动词实现 ───────────────────────────────────────────────

pub async fn list(gid: u32) -> Response {
    tokio::task::spawn_blocking(move || {
        if let Err(e) = ensure_ssh_dir(gid) {
            return Response::err(-1, e);
        }
        let mut keys: Vec<Value> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(ssh_dir()) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "pub").unwrap_or(false) {
                    if let Some(info) = parse_pub_file(&path) {
                        keys.push(info);
                    }
                }
            }
        }
        keys.sort_by(|a, b| {
            let a = a.get("created_at").and_then(|v| v.as_str()).unwrap_or("");
            let b = b.get("created_at").and_then(|v| v.as_str()).unwrap_or("");
            b.cmp(a)
        });
        Response::ok("ok", Some(json!(keys)))
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

pub async fn get(name: String, gid: u32) -> Response {
    tokio::task::spawn_blocking(move || {
        if !valid_name(&name) {
            return Response::err(-1, "无效的密钥名称");
        }
        let _ = ensure_ssh_dir(gid);
        let path = ssh_dir().join(format!("{name}.pub"));
        if !path.exists() {
            return Response::err(-1, "密钥不存在");
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => Response::ok(
                "ok",
                Some(json!({ "name": name, "public_key": content.trim() })),
            ),
            Err(e) => Response::err(-1, format!("读取失败: {e}")),
        }
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

pub async fn generate(
    name: String,
    key_type: Option<String>,
    bits: Option<u32>,
    comment: Option<String>,
    gid: u32,
) -> Response {
    tokio::task::spawn_blocking(move || {
        if !valid_name(&name) {
            return Response::err(-1, "无效的密钥名称");
        }
        if let Err(e) = ensure_ssh_dir(gid) {
            return Response::err(-1, e);
        }
        let dir = ssh_dir();
        let key_path = dir.join(&name);
        let pub_path = dir.join(format!("{name}.pub"));
        if key_path.exists() || pub_path.exists() {
            return Response::err(-1, "密钥已存在");
        }

        let key_type = key_type.unwrap_or_else(|| "ed25519".to_string());
        let comment = comment.unwrap_or_else(|| format!("{name}@zap"));
        let mut cmd = std::process::Command::new("ssh-keygen");
        cmd.args(["-t", &key_type])
            .args(["-f", &key_path.to_string_lossy()])
            .args(["-C", &comment])
            .args(["-N", ""])
            .arg("-q");
        if key_type == "rsa" {
            cmd.args(["-b", &bits.unwrap_or(4096).to_string()]);
        }

        match cmd.output() {
            Ok(o) if o.status.success() => {
                set_owned_mode(&key_path, gid, 0o640);
                set_owned_mode(&pub_path, gid, 0o640);
                Response::ok("密钥生成成功", Some(json!({ "name": name })))
            }
            Ok(o) => Response::err(
                -1,
                format!("密钥生成失败: {}", String::from_utf8_lossy(&o.stderr)),
            ),
            Err(e) => Response::err(-1, format!("ssh-keygen 执行失败: {e}")),
        }
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

pub async fn import(
    name: String,
    private_key: String,
    public_key: Option<String>,
    gid: u32,
) -> Response {
    tokio::task::spawn_blocking(move || {
        if !valid_name(&name) {
            return Response::err(-1, "无效的密钥名称");
        }
        if let Err(e) = ensure_ssh_dir(gid) {
            return Response::err(-1, e);
        }
        let dir = ssh_dir();
        let key_path = dir.join(&name);
        let pub_path = dir.join(format!("{name}.pub"));
        if key_path.exists() || pub_path.exists() {
            return Response::err(-1, "密钥已存在");
        }

        if let Err(e) = std::fs::write(&key_path, private_key.trim()) {
            return Response::err(-1, format!("写入私钥失败: {e}"));
        }
        if let Some(pk) = &public_key {
            if !pk.trim().is_empty() {
                let _ = std::fs::write(&pub_path, pk.trim());
            }
        }
        // 未提供公钥则从私钥派生
        if !pub_path.exists() {
            if let Ok(o) = std::process::Command::new("ssh-keygen")
                .args(["-y", "-f", &key_path.to_string_lossy()])
                .output()
            {
                if o.status.success() {
                    let _ = std::fs::write(&pub_path, String::from_utf8_lossy(&o.stdout).trim());
                }
            }
        }
        set_owned_mode(&key_path, gid, 0o640);
        set_owned_mode(&pub_path, gid, 0o640);
        Response::ok("密钥导入成功", Some(json!({ "name": name })))
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

pub async fn delete(name: String) -> Response {
    tokio::task::spawn_blocking(move || {
        if !valid_name(&name) {
            return Response::err(-1, "无效的密钥名称");
        }
        let dir = ssh_dir();
        let key_path = dir.join(&name);
        let pub_path = dir.join(format!("{name}.pub"));
        if !key_path.exists() && !pub_path.exists() {
            return Response::err(-1, "密钥不存在");
        }

        let pub_content = std::fs::read_to_string(&pub_path)
            .ok()
            .map(|s| s.trim().to_string());

        if key_path.exists() {
            if let Err(e) = std::fs::remove_file(&key_path) {
                return Response::err(-1, format!("删除失败: {e}"));
            }
        }
        if pub_path.exists() {
            let _ = std::fs::remove_file(&pub_path);
        }
        if let Some(pub_line) = pub_content {
            remove_from_authorized_keys(&pub_line);
        }
        Response::ok("删除成功", None)
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

pub async fn authorized_list() -> Response {
    tokio::task::spawn_blocking(|| {
        let path = ssh_dir().join("authorized_keys");
        if !path.exists() {
            return Response::ok("ok", Some(json!([])));
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let entries: Vec<Value> = content
                    .lines()
                    .enumerate()
                    .filter(|(_, l)| !l.trim().is_empty())
                    .map(|(i, line)| {
                        let parts: Vec<&str> = line.trim().splitn(3, ' ').collect();
                        json!({
                            "index": i,
                            "key_type": parts.first().unwrap_or(&""),
                            "key_data_short": parts.get(1)
                                .map(|d| if d.len() > 40 {
                                    format!("{}...", &d[..40])
                                } else {
                                    d.to_string()
                                })
                                .unwrap_or_default(),
                            "comment": parts.get(2).unwrap_or(&""),
                            "full_line": line.trim(),
                        })
                    })
                    .collect();
                Response::ok("ok", Some(json!(entries)))
            }
            Err(e) => Response::err(-1, format!("读取失败: {e}")),
        }
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

pub async fn authorize(name: String, gid: u32) -> Response {
    tokio::task::spawn_blocking(move || {
        if !valid_name(&name) {
            return Response::err(-1, "无效的密钥名称");
        }
        if let Err(e) = ensure_ssh_dir(gid) {
            return Response::err(-1, e);
        }
        let dir = ssh_dir();
        let pub_path = dir.join(format!("{name}.pub"));
        if !pub_path.exists() {
            return Response::err(-1, "公钥不存在，请先生成或导入密钥");
        }
        let pub_line = match std::fs::read_to_string(&pub_path) {
            Ok(c) => c.trim().to_string(),
            Err(e) => return Response::err(-1, format!("读取公钥失败: {e}")),
        };

        let auth_path = dir.join("authorized_keys");
        let mut existing = if auth_path.exists() {
            std::fs::read_to_string(&auth_path).unwrap_or_default()
        } else {
            String::new()
        };
        if existing.lines().any(|l| l.trim() == pub_line) {
            return Response::ok("该密钥已授权", None);
        }
        if !existing.is_empty() && !existing.ends_with('\n') {
            existing.push('\n');
        }
        existing.push_str(&pub_line);
        existing.push('\n');
        if let Err(e) = std::fs::write(&auth_path, &existing) {
            return Response::err(-1, format!("写入失败: {e}"));
        }
        set_owned_mode(&auth_path, gid, 0o640);
        Response::ok("授权成功", None)
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

pub async fn deauthorize(index: usize) -> Response {
    tokio::task::spawn_blocking(move || {
        let auth_path = ssh_dir().join("authorized_keys");
        if !auth_path.exists() {
            return Response::err(-1, "authorized_keys 不存在");
        }
        let content = match std::fs::read_to_string(&auth_path) {
            Ok(c) => c,
            Err(e) => return Response::err(-1, format!("读取失败: {e}")),
        };
        let lines: Vec<&str> = content.lines().collect();
        if index >= lines.len() {
            return Response::err(-1, "无效的索引");
        }
        let new_content: String = lines
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != index)
            .map(|(_, l)| *l)
            .collect::<Vec<_>>()
            .join("\n");
        let new_content = new_content.trim().to_string() + "\n";
        if let Err(e) = std::fs::write(&auth_path, new_content) {
            return Response::err(-1, format!("写入失败: {e}"));
        }
        Response::ok("取消授权成功", None)
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

// ── 本地用户 authorized_keys ────────────────────────────────

/// 校验系统用户名，防路径穿越。
fn valid_username(username: &str) -> bool {
    !username.is_empty()
        && username.len() <= 64
        && username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

/// 从 /etc/passwd 解析用户 uid/gid/home。
fn user_info(username: &str) -> Option<(u32, u32, PathBuf)> {
    if username == "root" {
        return Some((0, 0, PathBuf::from("/root")));
    }
    let content = std::fs::read_to_string("/etc/passwd").ok()?;
    for line in content.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() >= 6 && fields[0] == username {
            let uid: u32 = fields[2].parse().ok()?;
            let gid: u32 = fields[3].parse().ok()?;
            return Some((uid, gid, PathBuf::from(fields[5])));
        }
    }
    None
}

/// 把 `/etc/zap/ssh/<key_name>.pub` 追加到本机用户 `~/.ssh/authorized_keys`。
/// authorized_keys 属主必须为登录用户本人（sshd 严格校验），故这里用 root 设置属主后
/// 以 0600 保存，仅写入公钥，不涉及私钥。
pub async fn install_local(username: String, key_name: String) -> Response {
    tokio::task::spawn_blocking(move || {
        if !valid_username(&username) {
            return Response::err(-1, "无效的系统用户名");
        }
        if !valid_name(&key_name) {
            return Response::err(-1, "无效的密钥名称");
        }
        let pub_path = ssh_dir().join(format!("{key_name}.pub"));
        let pub_line = match std::fs::read_to_string(&pub_path) {
            Ok(c) => c.trim().to_string(),
            Err(_) => return Response::err(-1, "公钥不存在，请先生成或导入密钥"),
        };
        if pub_line.is_empty() {
            return Response::err(-1, "公钥内容为空");
        }
        let (uid, gid, home) = match user_info(&username) {
            Some(v) => v,
            None => return Response::err(-1, format!("系统用户 '{username}' 不存在")),
        };

        let ssh_dir = home.join(".ssh");
        if ssh_dir.exists() {
            let meta = match std::fs::metadata(&ssh_dir) {
                Ok(m) => m,
                Err(e) => return Response::err(-1, format!("读取 ~/.ssh 失败: {e}")),
            };
            if !meta.is_dir() {
                return Response::err(-1, "~/.ssh 不是目录");
            }
        } else if let Err(e) = std::fs::create_dir_all(&ssh_dir) {
            return Response::err(-1, format!("创建 ~/.ssh 失败: {e}"));
        }
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&ssh_dir, std::fs::Permissions::from_mode(0o700));
        if let Ok(c) = std::ffi::CString::new(ssh_dir.as_os_str().as_encoded_bytes()) {
            unsafe {
                libc::chown(c.as_ptr(), uid, gid);
            }
        }

        let auth_path = ssh_dir.join("authorized_keys");
        let mut existing = if auth_path.exists() {
            std::fs::read_to_string(&auth_path).unwrap_or_default()
        } else {
            String::new()
        };
        if existing.lines().any(|l| l.trim() == pub_line) {
            return Response::ok(format!("公钥已存在于 {username} 的 authorized_keys"), None);
        }
        if !existing.is_empty() && !existing.ends_with('\n') {
            existing.push('\n');
        }
        existing.push_str(&pub_line);
        existing.push('\n');
        if let Err(e) = std::fs::write(&auth_path, &existing) {
            return Response::err(-1, format!("写入 authorized_keys 失败: {e}"));
        }
        let _ = std::fs::set_permissions(&auth_path, std::fs::Permissions::from_mode(0o600));
        if let Ok(c) = std::ffi::CString::new(auth_path.as_os_str().as_encoded_bytes()) {
            unsafe {
                libc::chown(c.as_ptr(), uid, gid);
            }
        }
        Response::ok(
            format!("公钥已写入本机 {username} 的 ~/.ssh/authorized_keys"),
            None,
        )
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}
