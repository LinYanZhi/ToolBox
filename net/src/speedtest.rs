use std::io::Read;
use std::time::Instant;

use crate::agent::AgentConfig;

/// 测量指定 URL 的下载速度。
///
/// 主要方式：系统 curl（Schannel TLS，通过所有 CDN）。
/// 回退方式：ureq normal TLS → ureq insecure TLS → PowerShell Invoke-WebRequest。
///
/// PowerShell 使用 .NET HttpClient，TLS 指纹与 Rust/curl 不同，
/// 与下载逻辑的后端一致，减少测速结果显示不可用但实际能下载的情况。
///
/// 返回速度值，单位 KB/s。
pub fn measure_speed(url: &str, timeout_secs: u64) -> Option<f64> {
    // Primary: system curl.exe
    if let Some(speed) = crate::curl::try_curl_stdout(url, timeout_secs) {
        return Some(speed);
    }

    // Fallback: ureq
    const TEST_SIZE: usize = 64 * 1024;
    let normal = AgentConfig::normal(timeout_secs, timeout_secs);

    if let Some(speed) = try_fetch(&normal, url, TEST_SIZE, timeout_secs, true) {
        return Some(speed);
    }
    if let Some(speed) = try_fetch(&normal, url, TEST_SIZE, timeout_secs, false) {
        return Some(speed);
    }

    // insecure fallback
    let insecure = AgentConfig {
        insecure: true,
        connect_timeout: timeout_secs,
        read_timeout: timeout_secs,
        ..Default::default()
    };
    if let Some(speed) = try_fetch(&insecure, url, TEST_SIZE, timeout_secs, true) {
        return Some(speed);
    }
    if let Some(speed) = try_fetch(&insecure, url, TEST_SIZE, timeout_secs, false) {
        return Some(speed);
    }

    // PowerShell fallback — 使用 Windows 原生 HTTP 栈，可绕过 CDN 反爬
    if let Some(speed) = try_powershell_fetch(url, timeout_secs) {
        return Some(speed);
    }

    None
}

/// 使用 PowerShell Invoke-WebRequest 下载到临时文件并测量速度。
///
/// 与下载逻辑中的 PowerShell 后端一致，解决 curl/ureq 被 CDN 拦截时
/// 测速显示"不可用"但实际仍可通过 .NET HttpClient 下载的矛盾。
fn try_powershell_fetch(url: &str, timeout_secs: u64) -> Option<f64> {
    let tmp_dir = std::env::temp_dir();
    let tmp_file = tmp_dir.join(format!("as_speed_ps_{}.tmp", std::process::id()));

    let start = Instant::now();

    let ps_code = format!(
        "$ProgressPreference = 'SilentlyContinue'; \
try {{ \
  $cl = New-Object System.Net.WebClient; \
  $cl.Headers.Add('User-Agent', 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'); \
  $cl.Headers.Add('Accept', '*/*'); \
  $cl.DownloadFile('{}', '{}'); \
  exit 0 \
}} catch {{ exit 1 }}",
        url.replace('\'', "''"),
        tmp_file.to_string_lossy().replace('\'', "''")
    );

    let mut child = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_code])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    let deadline = Instant::now() + std::time::Duration::from_secs(timeout_secs);
    loop {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&tmp_file);
            return None;
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                let elapsed = start.elapsed().as_secs_f64();
                if !status.success() {
                    let _ = std::fs::remove_file(&tmp_file);
                    return None;
                }
                let size = std::fs::metadata(&tmp_file).map(|m| m.len()).unwrap_or(0);
                let _ = std::fs::remove_file(&tmp_file);
                if size < 1024 || elapsed < 0.1 {
                    return None;
                }
                return Some((size as f64 / 1024.0) / elapsed);
            }
            Ok(None) => {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(_) => {
                let _ = std::fs::remove_file(&tmp_file);
                return None;
            }
        }
    }
}

fn try_fetch(
    agent_cfg: &AgentConfig,
    url: &str,
    test_size: usize,
    timeout: u64,
    use_range: bool,
) -> Option<f64> {
    let start = Instant::now();
    let agent = agent_cfg.build_agent().ok()?;
    let mut req = agent.get(url);
    if use_range {
        req = req.set("Range", "bytes=0-65535");
    }
    let resp = agent_cfg.apply_headers(req, url).call().ok()?;
    let mut reader = resp.into_reader();
    let mut buf = [0u8; 16 * 1024];
    let mut total: usize = 0;

    loop {
        if start.elapsed().as_secs() >= timeout {
            if total > 0 {
                break;
            } else {
                return None;
            }
        }
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                total += n;
                if total >= test_size {
                    break;
                }
            }
            Err(_) => {
                if total == 0 {
                    return None;
                }
                break;
            }
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    if elapsed < 0.1 || total < 1024 {
        return None;
    }
    Some((total as f64 / 1024.0) / elapsed)
}
