use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{bail, Context};

use crate::agent::{AgentConfig, Fingerprint};
use crate::verify::verify_downloaded_file;

const CHUNK: usize = 64 * 1024;

/// 下载策略。
#[derive(Debug, Clone)]
pub enum DownloadStrategy {
    /// Rust 原生多线程 Range 分片下载。
    RustRange { threads: u8 },
    /// 使用系统 aria2c。
    Aria2c,
    /// 使用系统 curl。
    Curl,
    /// 使用 ureq 单线程。
    Ureq { fingerprint: Fingerprint, insecure: bool },
}

/// 下载配置。
#[derive(Debug, Clone)]
pub struct DownloadConfig {
    /// 策略回退链（按顺序尝试，直到成功）。
    pub strategies: Vec<DownloadStrategy>,
    /// 是否进行文件校验。
    pub verify: bool,
    /// 是否续传。
    pub resume: bool,
    /// 是否强制重新下载。
    pub renew: bool,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            strategies: vec![
                DownloadStrategy::RustRange { threads: 16 },
                DownloadStrategy::Aria2c,
                DownloadStrategy::Ureq { fingerprint: Fingerprint::Chrome120, insecure: false },
                DownloadStrategy::Ureq { fingerprint: Fingerprint::Chrome120, insecure: true },
                DownloadStrategy::Curl,
            ],
            verify: true,
            resume: true,
            renew: false,
        }
    }
}

impl DownloadConfig {
    /// 快速创建只包含指定策略的配置。
    pub fn with_strategies(strategies: Vec<DownloadStrategy>) -> Self {
        Self {
            strategies,
            ..Default::default()
        }
    }

    /// 设置强制重新下载。
    pub fn renew(mut self, renew: bool) -> Self {
        self.renew = renew;
        self
    }

    /// 设置文件校验。
    pub fn verify(mut self, verify: bool) -> Self {
        self.verify = verify;
        self
    }
}

/// 下载报告。
#[derive(Debug, Clone)]
pub struct DownloadReport {
    /// 最终成功使用的策略名称。
    pub strategy_used: &'static str,
    /// 下载的总字节数。
    pub total_bytes: u64,
    /// 耗时。
    pub elapsed: Duration,
    /// 下载的 URL。
    pub url: String,
    /// 目标路径。
    pub target_path: PathBuf,
}

/// 使用多策略回退链下载文件。
///
/// 按 `config.strategies` 中的策略顺序尝试，第一个成功的策略返回。
/// 所有策略都失败则返回最后一个错误。
pub fn download_with_fallback(
    url: &str,
    target_path: &Path,
    config: &DownloadConfig,
) -> anyhow::Result<DownloadReport> {
    let start = Instant::now();
    let mut errors: Vec<String> = Vec::new();

    for strategy in &config.strategies {
        let name = strategy_name(strategy);
        let result = match strategy {
            DownloadStrategy::RustRange { threads } => {
                download_with_range(url, target_path, *threads, config.resume)
                    .map(|_| name)
            }
            DownloadStrategy::Aria2c => {
                crate::aria2c::try_aria2c_download(url, target_path).map(|_| name)
            }
            DownloadStrategy::Curl => {
                crate::curl::try_curl_download(url, target_path).map(|_| name)
            }
            DownloadStrategy::Ureq { fingerprint, insecure } => {
                let agent_config = AgentConfig {
                    fingerprint: *fingerprint,
                    insecure: *insecure,
                    connect_timeout: 30,
                    read_timeout: 600,
                };
                if *insecure {
                    eprintln!("  ⚠ 正常 TLS 失败，尝试跳过证书验证（不安全）...");
                }
                download_with_ureq(url, target_path, &agent_config).map(|_| {
                    if *insecure { "ureq(insecure)" } else { "ureq" }
                })
            }
        };

        match result {
            Ok(strategy_name) => {
                if config.verify && !verify_downloaded_file(target_path) {
                    let msg = format!("{}: 下载内容签名不匹配", strategy_name);
                    errors.push(msg);
                    let _ = std::fs::remove_file(target_path);
                    continue;
                }
                let elapsed = start.elapsed();
                let total_bytes = std::fs::metadata(target_path).map(|m| m.len()).unwrap_or(0);
                return Ok(DownloadReport {
                    strategy_used: strategy_name,
                    total_bytes,
                    elapsed,
                    url: url.to_string(),
                    target_path: target_path.to_path_buf(),
                });
            }
            Err(e) => {
                errors.push(format!("{}: {}", name, e));
                let _ = std::fs::remove_file(target_path);
            }
        }
    }

    bail!("所有下载策略均失败: {}", errors.join("; "))
}

fn strategy_name(s: &DownloadStrategy) -> &'static str {
    match s {
        DownloadStrategy::RustRange { .. } => "RustRange",
        DownloadStrategy::Aria2c => "aria2c",
        DownloadStrategy::Curl => "curl",
        DownloadStrategy::Ureq { insecure: true, .. } => "ureq(insecure)",
        DownloadStrategy::Ureq { .. } => "ureq",
    }
}

/// Rust Range 分片下载。
fn download_with_range(url: &str, target_path: &Path, threads: u8, resume: bool) -> anyhow::Result<()> {
    crate::range::parallel_download(url, target_path, threads as usize, resume)
}

/// 使用 ureq Agent 单线程下载到文件。
fn download_with_ureq(url: &str, target_path: &Path, agent_cfg: &AgentConfig) -> anyhow::Result<()> {
    use std::io::Read;

    let agent = agent_cfg.build_agent()?;
    let mut req = agent.get(url);
    req = agent_cfg.apply_headers(req, url);

    let resp = req.call().context("ureq 请求失败")?;

    let status = resp.status();
    let ct = resp.header("Content-Type").unwrap_or("").to_string();

    // 非 2xx 状态码
    if status >= 400 {
        bail!("HTTP {} (Content-Type: {})", status, ct);
    }

    // 防盗链检测 — 给出更详细的诊断
    if crate::agent::is_html_response(&resp) {
        let ct_short = if ct.len() > 40 { format!("{}...", &ct[..40]) } else { ct.clone() };
        bail!(
            "服务器返回了 HTML 页面（可能反盗链），HTTP={} Content-Type={}",
            status, ct_short
        );
    }

    let _total_size: u64 = resp
        .header("Content-Length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let parent = target_path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;

    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(target_path)?;
    let mut buf = [0u8; CHUNK];

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
    }

    Ok(())
}

/// 对 GitHub 地址自动追加 ghproxy 镜像。
pub fn expand_github_urls(urls: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(urls.len() * 2);
    for url in urls {
        out.push(url.clone());
        if url.contains("github.com") && !url.contains("ghproxy") && !url.contains("gh-proxy") {
            out.push(format!("https://ghproxy.net/{}", url));
        }
    }
    out
}

/// 从多个 URL 中下载（顺序回退）。
pub fn download_with_url_fallback(
    name: &str,
    urls: &[String],
    target_path: &Path,
    config: &DownloadConfig,
) -> anyhow::Result<DownloadReport> {
    let expanded = expand_github_urls(urls);
    let mut last_err = None;

    for (i, url) in expanded.iter().enumerate() {
        let mut cfg = config.clone();
        // 只有第一个 URL 的首次尝试才启用 renew（避免重复下载）
        if i > 0 {
            cfg.renew = false;
        }
        match download_with_fallback(url, target_path, &cfg) {
            Ok(report) => return Ok(report),
            Err(e) => {
                last_err = Some(e);
                let _ = std::fs::remove_file(target_path);
            }
        }
    }

    let err = last_err.unwrap_or_else(|| anyhow::anyhow!("无可用下载地址"));
    bail!("{}: {}", name, err);
}
