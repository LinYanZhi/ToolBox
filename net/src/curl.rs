use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context};

use crate::download::Cancel;

/// 使用系统 curl 下载文件，先尝试正常模式，失败时尝试跳过证书验证。
/// 支持通过 Cancel 取消并自动清理临时文件。
pub fn try_curl_download(url: &str, target_path: &Path, cancel: &Cancel) -> anyhow::Result<()> {
    let curl = find_curl().ok_or_else(|| anyhow::anyhow!("未找到 curl.exe （不在 System32 或 PATH）"))?;

    match run_curl(&curl, url, target_path, false, cancel) {
        Ok(()) => return Ok(()),
        Err(_) => {}
    }

    match run_curl(&curl, url, target_path, true, cancel) {
        Ok(()) => Ok(()),
        Err(e) => bail!("curl 也失败 (含 --insecure): {}", e),
    }
}

fn run_curl(curl: &Path, url: &str, target_path: &Path, insecure: bool, cancel: &Cancel) -> anyhow::Result<()> {
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

    cmd.arg("--user-agent")
        .arg("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36");

    let hostname = url.split("://").nth(1).and_then(|s| s.split('/').next()).unwrap_or("");
    if !hostname.is_empty() {
        cmd.arg("--referer").arg(format!("https://{}/", hostname));
    }

    cmd.arg(url);

    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("启动 curl 失败")?;

    // ── 非阻塞等待 curl，支持取消 ──
    loop {
        if cancel.is_cancelled() {
            let _ = child.kill();
            let _ = std::fs::remove_file(target_path);
            return Err(anyhow::anyhow!("已取消"));
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if cancel.is_cancelled() {
                    let _ = std::fs::remove_file(target_path);
                    return Err(anyhow::anyhow!("已取消"));
                }
                if !status.success() {
                    let _ = std::fs::remove_file(target_path);
                    return Err(anyhow::anyhow!("curl 退出码 {}", status.code().unwrap_or(-1)));
                }
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(_) => {
                let _ = std::fs::remove_file(target_path);
                bail!("等待 curl 失败");
            }
        }
    }

    if !target_path.is_file() || std::fs::metadata(target_path).map(|m| m.len()).unwrap_or(0) == 0 {
        let _ = std::fs::remove_file(target_path);
        bail!("curl 下载的文件为空或不存在");
    }

    Ok(())
}

fn find_curl() -> Option<std::path::PathBuf> {
    if let Ok(path) = std::env::var("AMINOS_CURL_PATH") {
        let p = std::path::PathBuf::from(path);
        if p.is_file() { return Some(p); }
    }
    if let Some(system_root) = std::env::var_os("SystemRoot") {
        let system32 = std::path::PathBuf::from(system_root)
            .join("System32").join("curl.exe");
        if system32.is_file() { return Some(system32); }
    }
    std::env::var_os("PATH").and_then(|paths| {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join("curl.exe");
            if candidate.is_file() { return Some(candidate); }
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
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let elapsed = start.elapsed().as_secs_f64();
    let size = output.stdout.len();
    if elapsed < 0.1 || size < 1024 { return None; }
    Some((size as f64 / 1024.0) / elapsed)
}
