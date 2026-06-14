use std::io::Read;
use std::time::{Duration, Instant};

use crate::agent::AgentConfig;
use crate::download::Cancel;

/// 测速结果。
#[derive(Debug, Clone)]
pub struct ProbeReport {
    /// URL。
    pub url: String,
    /// HEAD 响应延迟。
    pub latency: Duration,
    /// Content-Length（0 表示未知）。
    pub content_length: u64,
    /// 小样本下载测吞度（字节/秒），0 表示未测。
    pub throughput: u64,
    /// 综合得分（越高越好）。
    pub score: u64,
}

/// 并发探测多个 URL，收集延迟 + 小样本测吞吐，按速度排序。
///
/// 比单纯的 HEAD 探测更精准——能检测出 CDN 对完整请求的限速。
pub fn probe_urls_ranked(urls: &[String], _timeout: Duration) -> Vec<String> {
    if urls.is_empty() {
        return vec![];
    }

    let cancel = Cancel::new();
    let results: Vec<ProbeReport> = urls.iter()
        .filter_map(|url| {
            let result = probe_single(url, &cancel);
            if result.is_some() { std::thread::sleep(Duration::from_millis(50)); }
            result
        })
        .collect();

    if results.is_empty() {
        return urls.to_vec();
    }

    // 按得分降序排列，得分相同按延迟升序
    let mut sorted = results;
    sorted.sort_by(|a, b| b.score.cmp(&a.score).then(a.latency.cmp(&b.latency)));

    sorted.into_iter().map(|r| r.url).collect()
}

/// 单 URL 探测：HEAD + 小样本下载测吞吐。
fn probe_single(url: &str, cancel: &Cancel) -> Option<ProbeReport> {
    let start = Instant::now();

    // ── HEAD 请求 ──
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(5))
        .user_agent(crate::agent::Fingerprint::Chrome120.user_agent())
        .build();

    let head_resp = agent.head(url).call().ok()?;
    if head_resp.status() >= 500 {
        return None;
    }

    let latency = start.elapsed();
    let content_length: u64 = head_resp
        .header("Content-Length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    // ── 小样本下载测吞吐（最多 256KB，最多 3 秒） ──
    let throughput = if content_length > 0 && latency < Duration::from_secs(3) {
        let sample_size = content_length.min(256 * 1024);
        measure_throughput(url, sample_size, cancel)
    } else {
        0
    };

    // ── 综合得分 ──
    // 吞吐量（KB/s）权重 70% + 延迟（ms 取倒数）权重 30%
    let throughput_score = if throughput > 0 {
        (throughput / 1024).min(1000) // 最高 1000 分
    } else {
        0
    };
    let latency_ms = latency.as_millis().max(1) as u64;
    let latency_score = (5000 / latency_ms).min(500); // 最高 500 分
    let score = throughput_score * 7 + latency_score * 3;

    Some(ProbeReport {
        url: url.to_string(),
        latency,
        content_length,
        throughput,
        score,
    })
}

/// 小样本下载测吞吐：下载前 `size` 字节，计算平均速度。
fn measure_throughput(url: &str, size: u64, cancel: &Cancel) -> u64 {
    let agent_cfg = AgentConfig {
        fingerprint: crate::agent::Fingerprint::Chrome120,
        insecure: false,
        connect_timeout: 5,
        read_timeout: 10,
    };
    let agent = match agent_cfg.build_agent() {
        Ok(a) => a,
        Err(_) => return 0,
    };

    let mut req = agent.get(url);
    req = agent_cfg.apply_headers(req, url);

    let resp = match req.call() {
        Ok(r) => r,
        Err(_) => return 0,
    };

    let start = Instant::now();
    let mut reader = resp.into_reader();
    let mut buf = vec![0u8; 65536];
    let mut downloaded = 0u64;

    loop {
        if cancel.is_cancelled() {
            return 0;
        }
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        downloaded += n as u64;
        if downloaded >= size {
            break;
        }
        if start.elapsed() > Duration::from_secs(3) {
            break;
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    if elapsed > 0.0 {
        (downloaded as f64 / elapsed) as u64
    } else {
        0
    }
}

/// 检查指定 URL 是否可达（只做 HEAD）。
pub fn is_reachable(url: &str) -> bool {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(5))
        .user_agent(crate::agent::Fingerprint::Chrome120.user_agent())
        .build();

    match agent.head(url).call() {
        Ok(resp) if resp.status() < 500 => true,
        _ => false,
    }
}
