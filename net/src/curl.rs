use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context};

/// 使用系统 curl 下载文件。
///
/// 优先调用 Windows 10 1809+ 内置的 `System32\curl.exe`，
/// 也会查找 PATH 中的 curl。
pub fn try_curl_download(url: &str, target_path: &Path) -> anyhow::Result<()> {
    let curl = find_curl().ok_or_else(|| anyhow::anyhow!("未找到 curl.exe"))?;

    let status = Command::new(&curl)
        .args([
            "-sL",
            "-o",
            &target_path.to_string_lossy(),
            "--max-time",
            "300",
            url,
        ])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("运行 curl 失败")?;

    if !status.success() {
        if target_path.exists()
            && target_path.metadata().map(|m| m.len() > 0).unwrap_or(false)
        {
            return Ok(());
        }
        bail!("curl 退出码 {}", status.code().unwrap_or(-1));
    }

    Ok(())
}

/// 在系统路径中查找 curl.exe。
fn find_curl() -> Option<std::path::PathBuf> {
    // 环境变量
    if let Ok(path) = std::env::var("AMINOS_CURL_PATH") {
        let p = std::path::PathBuf::from(path);
        if p.is_file() {
            return Some(p);
        }
    }

    // System32（Win10 1809+ / Win11 内置）
    if let Some(system_root) = std::env::var_os("SystemRoot") {
        let system32 = std::path::PathBuf::from(system_root)
            .join("System32")
            .join("curl.exe");
        if system32.is_file() {
            return Some(system32);
        }
    }

    // PATH
    std::env::var_os("PATH").and_then(|paths| {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join("curl.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    })
}

/// 使用 curl 测速：捕获 stdout 中的字节数/时间，返回 KB/s。
pub fn try_curl_stdout(url: &str, timeout_secs: u64) -> Option<f64> {
    let curl = find_curl()?;

    let max_time = timeout_secs + 5;
    let start = std::time::Instant::now();
    let output = Command::new(&curl)
        .args(["-sL", "-r", "0-65535", "--max-time", &max_time.to_string(), url])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    let elapsed = start.elapsed().as_secs_f64();
    let size = output.stdout.len();

    if elapsed < 0.1 || size < 1024 {
        return None;
    }
    Some((size as f64 / 1024.0) / elapsed)
}
