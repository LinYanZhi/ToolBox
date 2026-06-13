use std::io::Read;
use std::time::Instant;

use crate::agent::AgentConfig;

/// 测量指定 URL 的下载速度。
///
/// 主要方式：系统 curl（Schannel TLS，通过所有 CDN）。
/// 回退方式：ureq normal TLS → ureq insecure TLS。
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

    None
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
