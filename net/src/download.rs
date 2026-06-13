use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use anyhow::bail;
use indicatif::MultiProgress;

use crate::agent::{AgentConfig, Fingerprint};
use crate::verify::verify_downloaded_file;

const CHUNK: usize = 64 * 1024;

/// 全局进度条管理器，确保同一时间多个进度条不会在终端打架。
pub(crate) fn progress() -> &'static MultiProgress {
    static MP: OnceLock<MultiProgress> = OnceLock::new();
    MP.get_or_init(MultiProgress::new)
}

/// 取消令牌，用于优雅终止后台下载线程。
#[derive(Clone)]
pub struct Cancel(Arc<CancelInner>);

pub(crate) struct CancelInner {
    cancelled: AtomicBool,
    /// Unix 毫秒时间戳，0 表示从未有过进度
    last_progress: AtomicU64,
}

impl Cancel {
    pub fn new() -> Self {
        Self(Arc::new(CancelInner {
            cancelled: AtomicBool::new(false),
            last_progress: AtomicU64::new(0),
        }))
    }
    /// 请求取消
    pub fn cancel(&self) { self.0.cancelled.store(true, Ordering::Relaxed); }
    /// 是否已被取消
    pub fn is_cancelled(&self) -> bool { self.0.cancelled.load(Ordering::Relaxed) }
    /// 标记已收到数据
    pub fn mark_progress(&self) {
        self.0.last_progress.store(unix_ms(), Ordering::Relaxed);
    }
    /// 上次收到数据的时间戳（unix ms），0 表示从未
    pub fn last_progress_ms(&self) -> u64 {
        self.0.last_progress.load(Ordering::Relaxed)
    }
}

/// 当前 Unix 毫秒时间戳。
fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

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
    pub strategy_used: String,
    /// 下载的总字节数。
    pub total_bytes: u64,
    /// 耗时。
    pub elapsed: Duration,
    /// 下载的 URL。
    pub url: String,
    /// 目标路径。
    pub target_path: PathBuf,
}

/// 清理所有策略留下的临时文件（`{target}.strategy_*.tmp`、`*.parts.*` 目录等）。
fn cleanup_strategy_temp(target_path: &Path) {
    let parent = target_path.parent().unwrap_or(Path::new("."));
    let stem = target_path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    if stem.is_empty() {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            // 匹配 {target}.strategy_N.tmp 或 {target}.strategy_N.tmp.parts/ 或 {target}.parts.NNNN/
            if name.starts_with(&stem) && (name.contains(".strategy_") || name.contains(".parts.")) {
                let _ = if path.is_dir() {
                    std::fs::remove_dir_all(&path)
                } else {
                    std::fs::remove_file(&path)
                };
            }
        }
    }
}

/// 从 URL 中提取简短标签（用于进度条显示）。
fn url_label(url: &str) -> &str {
    // 尝试提取有意义的短标签
    if url.contains("ghproxy.net") { return "ghproxy"; }
    if url.contains("cdn.jsdelivr.net") { return "jsdelivr"; }
    if url.contains("raw.githubusercontent.com") { return "raw"; }
    if url.contains("github.com") { return "github"; }
    // 取 hostname
    let cleaned = url.trim_start_matches("https://").trim_start_matches("http://");
    if let Some(pos) = cleaned.find('/') {
        let host = &cleaned[..pos];
        if let Some(dot) = host.rfind('.') {
            let prev_dot = host[..dot].rfind('.');
            let start = prev_dot.map(|p| p + 1).unwrap_or(0);
            return &host[start..dot];
        }
        return host;
    }
    "?source"
}

/// 并行下载：多个 URL × 多个策略同时跑，谁先成功用谁。
///
/// 每个 (URL, 策略) 组合有独立的临时文件 `{target}.s{src_idx}_{strat_idx}.tmp`
/// 和一个独立的进度条。第一个成功的策略胜出，其余被取消。
pub fn download_with_fallback(
    urls: &[String],
    target_path: &Path,
    config: &DownloadConfig,
) -> anyhow::Result<DownloadReport> {
    let start = Instant::now();
    let total_combos = urls.len() * config.strategies.len();

    if urls.is_empty() {
        bail!("没有可用的下载地址");
    }

    // ── 1. 构建组合列表 ──
    struct Combo {
        url: String,
        strategy: DownloadStrategy,
        src_idx: usize,
        strat_idx: usize,
        label: String,
    }

    let combos: Vec<Combo> = urls.iter().enumerate().flat_map(|(si, url)| {
        let src_label = url_label(url);
        config.strategies.iter().enumerate().map(move |(ti, strategy)| {
            let sname = strategy_name(strategy);
            Combo {
                url: url.clone(),
                strategy: strategy.clone(),
                src_idx: si,
                strat_idx: ti,
                label: format!("[{}] {}", src_label, sname),
            }
        })
    }).collect();

    eprintln!("    ⚡ 并行尝试 {} 种组合...", total_combos);

    // ── 2. 创建所有组合的进度条 ──
    let progress_style = indicatif::ProgressStyle::default_bar()
        .template("{msg:.bold} [{bar:20}] {bytes}/{total_bytes} ({bytes_per_sec})")
        .unwrap()
        .progress_chars("=> ");

    let combo_bars: Vec<indicatif::ProgressBar> = combos.iter().map(|c| {
        let bar = crate::download::progress().add(indicatif::ProgressBar::new(1));
        bar.set_style(progress_style.clone());
        bar.set_message(format!("{} 等待中...", c.label));
        bar.set_length(1);
        bar.set_position(0);
        bar
    }).collect();

    // ── 3. 并行启动所有组合 ──
    // 共享状态：赢家标记、取消句柄
    let someone_won = Arc::new(AtomicBool::new(false));
    let (tx, rx) = std::sync::mpsc::channel::<(String, String, u64)>(); // (strategy_name, url, file_size)
    let mut cancel_handles: Vec<crate::download::Cancel> = Vec::with_capacity(combos.len());

    for (idx, combo) in combos.into_iter().enumerate() {
        let tmp_path = target_path.with_extension(format!("s{}_{}.tmp", combo.src_idx, combo.strat_idx));
        let cancel = Cancel::new();
        cancel_handles.push(cancel.clone());

        let combo_url = combo.url.clone();
        let combo_strategy = combo.strategy;
        let combo_label = combo.label.clone();
        let bar = combo_bars[idx].clone();
        let tx = tx.clone();
        let someone_won = Arc::clone(&someone_won);
        let resume = config.resume;

        // 更新进度条为"运行中"
        bar.set_message(format!("{} 运行中...", combo_label));

        std::thread::spawn(move || {
            // 如果已经有胜出者，直接退出
            if someone_won.load(Ordering::Relaxed) {
                bar.finish_and_clear();
                let _ = std::fs::remove_file(&tmp_path);
                return;
            }

            let result = match &combo_strategy {
                DownloadStrategy::RustRange { threads } => {
                    download_with_range(&combo_url, &tmp_path, *threads, resume, &cancel, Some(bar.clone()))
                        .map(|_| "RustRange")
                }
                DownloadStrategy::Aria2c => {
                    crate::aria2c::try_aria2c_download(&combo_url, &tmp_path)
                        .map(|_| "aria2c")
                }
                DownloadStrategy::Curl => {
                    crate::curl::try_curl_download(&combo_url, &tmp_path)
                        .map(|_| "curl")
                }
                DownloadStrategy::PowerShell => {
                    crate::powershell::try_powershell_download(&combo_url, &tmp_path)
                        .map(|_| "powershell")
                }
                DownloadStrategy::PowerShellInvoke => {
                    crate::powershell::try_powershell_invoke(&combo_url, &tmp_path)
                        .map(|_| "powershell-invoke")
                }
                DownloadStrategy::BitsTransfer => {
                    crate::powershell::try_bits_transfer(&combo_url, &tmp_path)
                        .map(|_| "bits")
                }
                DownloadStrategy::Ureq { fingerprint, insecure } => {
                    let agent_config = AgentConfig {
                        fingerprint: *fingerprint,
                        insecure: *insecure,
                        connect_timeout: 15,
                        read_timeout: 600,
                    };
                    download_with_ureq(&combo_url, &tmp_path, &agent_config, &cancel, Some(bar.clone()))
                        .map(|_| {
                            if *insecure { "ureq(insecure)" } else { "ureq" }
                        })
                }
            };

            match result {
                Ok(strategy_name) => {
                    // 标记胜出
                    if someone_won.swap(true, Ordering::Relaxed) {
                        // 已经有胜出者了，清理退出
                        bar.finish_and_clear();
                        let _ = std::fs::remove_file(&tmp_path);
                        return;
                    }

                    // 胜出！发送报告
                    let file_size = std::fs::metadata(&tmp_path).map(|m| m.len()).unwrap_or(0);
                    bar.finish_with_message(format!("✓ {} (done)", combo_label));
                    let _ = tx.send((strategy_name.to_string(), combo_url, file_size));
                }
                Err(e) => {
                    if someone_won.load(Ordering::Relaxed) {
                        bar.finish_and_clear();
                    } else {
                        bar.finish_with_message(format!("✗ {} ({})", combo_label, e));
                    }
                    let _ = std::fs::remove_file(&tmp_path);
                }
            }
        });
    }

    // ── 4. 等待第一个成功，或全部失败 ──
    const FIRST_BYTE_TIMEOUT: Duration = Duration::from_secs(15);
    const STALL_TIMEOUT: Duration = Duration::from_secs(20);

    let result = loop {
        match rx.recv_timeout(FIRST_BYTE_TIMEOUT) {
            Ok((strategy_name, url, file_size)) => {
                break Ok((strategy_name, url, file_size));
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // 检查是否有任何组合有进度
                let now = unix_ms();
                let any_alive = cancel_handles.iter().any(|c| {
                    let last = c.last_progress_ms();
                    last > 0 && now.saturating_sub(last) < STALL_TIMEOUT.as_millis() as u64
                });

                if !any_alive {
                    break Err(anyhow::anyhow!("所有下载组合均无响应"));
                }
                // 还有组合在跑，继续等
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break Err(anyhow::anyhow!("所有下载组合均失败"));
            }
        }
    };

    // ── 5. 取消所有仍在运行的组合 ──
    for c in &cancel_handles {
        c.cancel();
    }

    // ── 6. 处理结果 ──
    match result {
        Ok((strategy_name, url, _file_size)) => {
            // 扫描找到胜出的临时文件并 rename
            let parent = target_path.parent().unwrap_or(Path::new("."));
            if let Ok(entries) = std::fs::read_dir(parent) {
                let stem = target_path.file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = path.file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    if name.starts_with(&stem) && name.ends_with(".tmp") {
                        let _ = std::fs::remove_file(target_path);
                        if std::fs::rename(&path, target_path).is_ok() {
                            break;
                        }
                    }
                }
            }

            if !target_path.is_file() {
                cleanup_strategy_temp(target_path);
                bail!("胜出组合的临时文件不存在");
            }

            // 校验
            if config.verify && !verify_downloaded_file(target_path) {
                let _ = std::fs::remove_file(target_path);
                cleanup_strategy_temp(target_path);
                bail!("{}: 下载内容签名不匹配", strategy_name);
            }

            // 清理临时文件
            cleanup_strategy_temp(target_path);

            // 报告
            let file_size = std::fs::metadata(target_path).map(|m| m.len()).unwrap_or(0);
            let elapsed = start.elapsed();
            eprintln!("    ✓ 成功: {} ({}, {} KB/s)",
                strategy_name,
                color::format_size(file_size),
                if elapsed.as_secs() > 0 { file_size / 1024 / elapsed.as_secs() as u64 } else { 0 }
            );

            Ok(DownloadReport {
                strategy_used: strategy_name.clone(),
                total_bytes: file_size,
                elapsed,
                url,
                target_path: target_path.to_path_buf(),
            })
        }
        Err(e) => {
            cleanup_strategy_temp(target_path);
            bail!("下载失败: {}", e);
        }
    }
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
fn download_with_range(url: &str, target_path: &Path, threads: u8, resume: bool, cancel: &Cancel, pb: Option<indicatif::ProgressBar>) -> anyhow::Result<()> {
    crate::range::parallel_download(url, target_path, threads as usize, resume, cancel, pb)
}

/// 使用 ureq Agent 单线程下载到文件（含进度条 + cookie 挑战回退）。
fn download_with_ureq(
    url: &str,
    target_path: &Path,
    agent_cfg: &AgentConfig,
    cancel: &Cancel,
    external_pb: Option<indicatif::ProgressBar>,
) -> anyhow::Result<()> {
    let agent = agent_cfg.build_agent()?;

    // 最多尝试 3 次（正常 → cookie 回退 → URL 清理）
    let mut last_err = None;
    let mut cookie: Option<String> = None;
    let mut current_url = url.to_string();

    for attempt in 0..3 {
        // 检查取消令牌
        if cancel.is_cancelled() {
            return Err(anyhow::anyhow!("已取消"));
        }

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

        let pb: Option<indicatif::ProgressBar> = if let Some(bar) = external_pb {
            if total > 0 {
                bar.set_length(total);
            }
            bar.set_message("下载中");
            Some(bar)
        } else if total > 0 {
            let bar = progress().add(indicatif::ProgressBar::new(total));
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
            if cancel.is_cancelled() {
                // 清理进度条和临时文件
                if let Some(ref pb) = pb {
                    pb.finish_and_clear();
                }
                let _ = std::fs::remove_file(target_path);
                return Err(anyhow::anyhow!("已取消"));
            }
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])?;
            cancel.mark_progress();
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

/// 对 GitHub 地址自动追加 ghproxy 镜像，ghproxy 排在原链前面。
pub fn expand_github_urls(urls: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(urls.len() * 2);
    for url in urls {
        let is_github = url.contains("github.com") || url.contains("raw.githubusercontent.com");
        if is_github && !url.contains("ghproxy") && !url.contains("gh-proxy") {
            // ghproxy 优先于原链
            out.push(format!("https://ghproxy.net/{}", url));
            out.push(url.clone());
        } else {
            out.push(url.clone());
        }
    }
    out
}

/// 并发探测多个 URL，返回 8s 内确认可用的地址列表。
///
/// 快速过滤死链，避免无效组合浪费带宽。
fn probe_alive_urls(urls: &[String]) -> Vec<String> {
    let total = urls.len();
    if total == 0 { return vec![]; }

    eprintln!("    ⚡ 并行探测 {} 个地址（8s 封顶）", total);

    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let probe_timeout = Duration::from_secs(8);

    for url in urls {
        let tx = tx.clone();
        let url = url.clone();
        std::thread::spawn(move || {
            let agent = ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_secs(5))
                .timeout_read(Duration::from_secs(5))
                .user_agent("aminos/0.1")
                .build();

            match agent.head(&url).call() {
                Ok(resp) if resp.status() < 400 => {
                    let ct = resp.header("Content-Type").unwrap_or("");
                    if !ct.contains("text/html") {
                        let _ = tx.send(url);
                    }
                }
                _ => {}
            }
        });
    }

    drop(tx);
    let mut alive = Vec::new();
    let deadline = Instant::now() + probe_timeout;
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(url) => alive.push(url),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if alive.is_empty() { continue; }
                // 已有探测结果，可以继续等满时间收集更多
                continue;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    alive
}

/// 从多个 URL 中下载（智能并发探测 + 并行多策略）。
///
/// 第一阶段：并行探测所有 URL，收集可达地址。
/// 第二阶段：所有可达地址 × 所有策略并行下载，谁先完成用谁。
pub fn download_with_url_fallback(
    _name: &str,
    urls: &[String],
    target_path: &Path,
    config: &DownloadConfig,
) -> anyhow::Result<DownloadReport> {
    let expanded = expand_github_urls(urls);

    // ── Phase 1: 并行探测，收集可达地址 ──
    let alive = probe_alive_urls(&expanded);
    let alive = if alive.is_empty() {
        eprintln!("    ⚠ 全部探测无响应（8s 超时），降级为全部地址参与");
        expanded
    } else {
        alive
    };

    // ── Phase 2: 并行下载（所有可达地址 × 全部策略） ──
    download_with_fallback(&alive, target_path, config)
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
