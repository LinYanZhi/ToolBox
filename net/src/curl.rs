use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context};

/// 使用系统 curl 下载文件，先尝试正常模式，失败时尝试跳过证书验证。
///
/// 优先调用 Windows 10 1809+ 内置的 `System32\curl.exe`，
/// 也会查找 PATH 中的 curl。
pub fn try_curl_download(url: &str, target_path: &Path) -> anyhow::Result<()> {
    let curl = find_curl().ok_or_else(|| anyhow::anyhow!("未找到 curl.exe （不在 System32 或 PATH）"))?;

    // 先尝试正常模式
    match run_curl(&curl, url, target_path, false) {
        Ok(()) => return Ok(()),
        Err(e) => {
            eprintln!("       curl 正常模式失败: {}", e);
        }
    }

    // 回退: --insecure
    eprintln!("       curl 尝试跳过证书验证...");
    match run_curl(&curl, url, target_path, true) {
        Ok(()) => Ok(()),
        Err(e) => {
            bail!("curl 也失败 (含 --insecure): {}", e);
        }
    }
}

fn run_curl(curl: &Path, url: &str, target_path: &Path, insecure: bool) -> anyhow::Result<()> {
    let parent = target_path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;

    let target_str = target_path.to_string_lossy().to_string();

    let mut cmd = Command::new(curl);
    cmd.arg("-sL")
        .arg("-o")
        .arg(&target_str)
        .arg("--max-time")
        .arg("300");

    if insecure {
        cmd.arg("--insecure");
    }

    // 传 Referer 和 User-Agent
    cmd.arg("--user-agent")
        .arg("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36");

    let hostname = url.split("://").nth(1).and_then(|s| s.split('/').next()).unwrap_or("");
    if !hostname.is_empty() {
        cmd.arg("--referer").arg(format!("https://{}/", hostname));
    }

    cmd.arg(url);

    let output = cmd
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .context("运行 curl 失败")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(anyhow::anyhow!("curl 退出码 {}: {}",
            output.status.code().unwrap_or(-1),
            if stderr.is_empty() { "无错误信息" } else { &stderr }
        ));
    }

    if !target_path.is_file() || std::fs::metadata(target_path).map(|m| m.len()).unwrap_or(0) == 0 {
        bail!("curl 下载的文件为空或不存在");
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
