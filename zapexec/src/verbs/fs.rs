//! 受限的文件系统只读操作（root 执行）。
//!
//! 当前只有 `browse_dirs`：列出某个目录下的直接子目录（供站点「选择已有站点目录」）。
//! 安全边界：只读目录项、只返回目录名（不返回文件/内容），
//! 路径须为绝对路径；调用方（zapd）在传入前已限定为归属用户家目录内。

use std::path::Path;

use serde_json::json;
use zap_proto::Response;

/// 列出 `base` 下的直接子目录名（字典序；不含点目录；跳过符号链接目录）。
/// base 不存在 / 非目录时返回错误。
pub async fn browse_dirs(base: String) -> Response {
    tokio::task::spawn_blocking(move || -> Result<Response, String> {
        if !base.starts_with('/') {
            return Err("浏览路径必须是绝对路径".to_string());
        }
        if base.split('/').any(|s| s == "..") {
            return Err("浏览路径不允许包含 ..".to_string());
        }
        let p = Path::new(&base);
        if !p.is_dir() {
            return Err(format!("目录不存在或不是目录：{base}"));
        }
        let mut dirs: Vec<String> = Vec::new();
        for entry in std::fs::read_dir(p)
            .map_err(|e| format!("读取目录失败：{e}"))?
            .flatten()
        {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let ft = entry
                .file_type()
                .map_err(|e| format!("读取目录项类型失败：{e}"))?;
            // 只列目录；符号链接一律跳过（避免被链到站外目录的目录误导/越权浏览）
            if ft.is_dir() {
                dirs.push(name);
            }
        }
        dirs.sort();
        Ok(Response::ok(
            "OK",
            Some(json!({ "base": base, "dirs": dirs })),
        ))
    })
    .await
    .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
    .unwrap_or_else(|e| Response::err(-1, e))
}
