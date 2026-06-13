use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::bail;

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
    /// 使用 Windows PowerShell (WebClient) — 不同 TLS 指纹。
    PowerShell,
    /// 使用 Windows PowerShell (Invoke-WebRequest) — 更完整的请求。
    PowerShellInvoke,
    /// 使用 Windows BITS 传输。
    BitsTransfer,
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
                DownloadStrategy::PowerShell,
                DownloadStrategy::PowerShellInvoke,
                DownloadStrategy::BitsTransfer,
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
    let total = config.strategies.len();

    for (idx, strategy) in config.strategies.iter().enumerate() {
        let name = strategy_name(strategy);
        eprintln!("  [{}/{}] 尝试 {} ...", idx + 1, total, name);

        // 对各别策略加简短提示
        match strategy {
            DownloadStrategy::Ureq { insecure: true, .. } => {
                eprintln!("       ⚠ 跳过证书验证（不安全）");
            }
            _ => {}
        }

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
            DownloadStrategy::PowerShell => {
                crate::powershell::try_powershell_download(url, target_path)
                    .map(|_| "powershell")
            }
            DownloadStrategy::PowerShellInvoke => {
                crate::powershell::try_powershell_invoke(url, target_path)
                    .map(|_| "powershell-invoke")
            }
            DownloadStrategy::BitsTransfer => {
                crate::powershell::try_bits_transfer(url, target_path)
                    .map(|_| "bits")
            }
            DownloadStrategy::Ureq { fingerprint, insecure } => {
                let agent_config = AgentConfig {
                    fingerprint: *fingerprint,
                    insecure: *insecure,
                    connect_timeout: 30,
                    read_timeout: 600,
                };
                download_with_ureq(url, target_path, &agent_config).map(|_| {
                    if *insecure { "ureq(insecure)" } else { "ureq" }
                })
            }
        };

        match result {
            Ok(strategy_name) => {
                if config.verify && !verify_downloaded_file(target_path) {
                    let msg = format!("{}: 下载内容签名不匹配（文件损坏或反盗链页面）", strategy_name);
                    eprintln!("       {}", msg);
                    errors.push(msg);
                    let _ = std::fs::remove_file(target_path);
                    let parts_dir = format!("{}.parts", target_path.display());
                    let _ = std::fs::remove_dir_all(&parts_dir);
                    continue;
                }
                let file_size = std::fs::metadata(target_path).map(|m| m.len()).unwrap_or(0);
                let elapsed = start.elapsed();
                eprintln!("       ✓ 成功 ({}, {} KB/s)",
                    color::format_size(file_size),
                    if elapsed.as_secs() > 0 { file_size / 1024 / elapsed.as_secs() as u64 } else { 0 }
                );
                return Ok(DownloadReport {
                    strategy_used: strategy_name,
                    total_bytes: file_size,
                    elapsed,
                    url: url.to_string(),
                    target_path: target_path.to_path_buf(),
                });
            }
            Err(e) => {
                let msg = format!("{}: {}", name, e);
                eprintln!("       ✗ {}", msg);
                errors.push(msg);
                let _ = std::fs::remove_file(target_path);
                let parts_dir = format!("{}.parts", target_path.display());
                let _ = std::fs::remove_dir_all(&parts_dir);
            }
        }
    }

    eprintln!("  ────────────────────────────────────");
    eprintln!("  所有 {} 种下载策略均失败", total);
    for err in &errors {
        eprintln!("    {}", err);
    }
    bail!("下载失败：{}", errors.last().unwrap_or(&"未知错误".to_string()));
}

fn strategy_name(s: &DownloadStrategy) -> &'static str {
    match s {
        DownloadStrategy::RustRange { .. } => "RustRange",
        DownloadStrategy::Aria2c => "aria2c",
        DownloadStrategy::Curl => "curl",
        DownloadStrategy::PowerShell => "powershell",
        DownloadStrategy::PowerShellInvoke => "powershell-invoke",
        DownloadStrategy::BitsTransfer => "bits",
        DownloadStrategy::Ureq { insecure: true, .. } => "ureq(insecure)",
        DownloadStrategy::Ureq { .. } => "ureq",
    }
}

/// Rust Range 分片下载。
fn download_with_range(url: &str, target_path: &Path, threads: u8, resume: bool) -> anyhow::Result<()> {
    crate::range::parallel_download(url, target_path, threads as usize, resume)
}

/// 使用 ureq Agent 单线程下载到文件（含进度条 + cookie 挑战回退）。
fn download_with_ureq(url: &str, target_path: &Path, agent_cfg: &AgentConfig) -> anyhow::Result<()> {
    let agent = agent_cfg.build_agent()?;

    // 最多尝试 3 次（正常 → cookie 回退 → URL 清理）
    let mut last_err = None;
    let mut cookie: Option<String> = None;
    let mut current_url = url.to_string();

    for attempt in 0..3 {
        let mut req = agent.get(&current_url);
        req = agent_cfg.apply_headers(req, &current_url);
        if let Some(ref c) = cookie {
            req = req.set("Cookie", c);
        }

        let resp = match req.call() {
            Ok(r) => r,
            Err(e) => {
                last_err = Some(anyhow::anyhow!("ureq 请求失败: {}", e));
                continue;
            }
        };

        let status = resp.status();
        let ct = resp.header("Content-Type").unwrap_or("").to_string();

        // 非 2xx 状态码
        if status >= 400 {
            last_err = Some(anyhow::anyhow!("HTTP {} (Content-Type: {})", status, ct));
            continue;
        }

        // 防盗链检测
        let is_html = crate::agent::is_html_response(&resp);

        if is_html {
            // 尝试提取 Set-Cookie
            let set_cookie = resp.header("set-cookie").map(|s| s.to_string());
            let had_cookie = cookie.is_some();

            if attempt == 0 && set_cookie.is_some() {
                // 首次遇到 HTML + Set-Cookie → cookie 挑战
                cookie = Some(set_cookie.unwrap());
                eprintln!("       ⚠ Cookie 挑战，重试中...");
                last_err = None;
                continue;
            }

            if attempt == 1 && !had_cookie {
                // 尝试剥离 tads 参数（JS 挑战的清理逻辑）
                if let Some(pos) = current_url.find("?tads") {
                    current_url = current_url[..pos].to_string();
                    eprintln!("       ⚠ 清理 URL 参数重试...");
                    last_err = None;
                    continue;
                }
                if let Some(pos) = current_url.find("&tads") {
                    current_url = current_url[..pos].to_string();
                    eprintln!("       ⚠ 清理 URL 参数重试...");
                    last_err = None;
                    continue;
                }
            }

            let ct_short = if ct.len() > 40 { format!("{}...", &ct[..40]) } else { ct.clone() };
            last_err = Some(anyhow::anyhow!(
                "服务器返回了 HTML 页面（可能反盗链），HTTP={} Content-Type={}",
                status, ct_short
            ));
            continue;
        }

        // 成功：非 HTML 响应 → 下载到文件
        let total: u64 = resp
            .header("Content-Length")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let parent = target_path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)?;

        let mut reader = resp.into_reader();
        let mut file = std::fs::File::create(target_path)?;
        let mut buf = vec![0u8; CHUNK];

        let pb = if total > 0 {
            let bar = indicatif::ProgressBar::new(total);
            bar.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template("{msg:.green} [{bar:30}] {bytes}/{total_bytes} ({bytes_per_sec})")
                    .unwrap()
                    .progress_chars("=> "),
            );
            bar.set_message("下载中");
            Some(bar)
        } else {
            None
        };

        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])?;
            if let Some(ref pb) = pb {
                pb.inc(n as u64);
            }
        }

        if let Some(pb) = pb {
            pb.finish_with_message("下载完成");
        }

        return Ok(());
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("ureq 下载失败")))
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

/// 通过 HEAD 请求探测下载 URL 的真实文件名。
///
/// 跟随重定向到最终 URL，依次检查：
/// 1. `Content-Disposition` 响应头中的 `filename` 字段
/// 2. 最终 URL 路径的最后一段（如 `/download/snipaste-2.11.3.zip` → `snipaste-2.11.3.zip`）
///
/// 如果 URL 路径本身已有明确的文件扩展名，直接返回路径末尾的文件名，不发 HEAD 请求。
/// 可用于下载前确定正确文件名，避免依赖魔数修正。
pub fn probe_filename(url: &str) -> Option<String> {
    // 先检查 URL 路径：如果已有已知扩展名，直接取路径末尾文件名
    let path = url.split('?').next().unwrap_or(url);
    let seg = path.rsplit('/').next().filter(|s| !s.is_empty())?;
    if let Some(dot) = seg.rfind('.') {
        let ext = &seg[dot..];
        if [
            ".exe", ".msi", ".zip", ".7z", ".rar", ".tar", ".gz", ".xz", ".bz2", ".iso",
            ".appx", ".dmg", ".cab", ".run",
        ]
        .contains(&ext.to_lowercase().as_str())
        {
            return Some(seg.to_string());
        }
    }

    // URL 无扩展名 → 发 HEAD 探测（Content-Disposition 或重定向后路径）
    let agent = crate::agent::AgentConfig::normal(15, 15).build_agent().ok()?;
    let resp = agent.head(url).call().ok()?;

    // 1. Content-Disposition header
    if let Some(cd) = resp.header("Content-Disposition") {
        for part in cd.split(';') {
            let part = part.trim();
            if let Some(val) = part.strip_prefix("filename=") {
                let name = val.trim_matches('"').trim_matches('\'').trim();
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
    }

    // 2. 最终 URL 路径（跟随重定向后）
    let final_url = resp.get_url();
    let seg = final_url.rsplit('/').next().filter(|s| !s.is_empty())?;
    if seg.contains('.') {
        Some(seg.to_string())
    } else {
        None
    }
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
        eprintln!("    URL[{}]: {}", i + 1, url);
        let mut cfg = config.clone();
        // 只有第一个 URL 的首次尝试才启用 renew（避免重复下载）
        if i > 0 {
            cfg.renew = false;
        }
        match download_with_fallback(url, target_path, &cfg) {
            Ok(report) => return Ok(report),
            Err(e) => {
                eprintln!("    \u{2717} 该地址失败");
                last_err = Some(e);
                let _ = std::fs::remove_file(target_path);
            }
        }
    }

    let err = last_err.unwrap_or_else(|| anyhow::anyhow!("无可用下载地址"));
    bail!("{}: {}", name, err);
}
