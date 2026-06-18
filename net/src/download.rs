use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use color::*;
use indicatif::MultiProgress;

use crate::backend::DownloadBackend;

use crate::agent::{AgentConfig, Fingerprint};
use crate::verify::verify_downloaded_file;

const CHUNK: usize = 64 * 1024;

/// 格式化下载进度为 "cur/total UNIT"（1 位小数，十进制，无末尾零）。
/// 例: "12.5/12.5 MB"，"1.2/1.2 GB"
pub(crate) fn format_decimal_progress(cur: u64, total: u64) -> String {
    let (cur_f, total_f, unit) = if total >= 1_000_000_000 {
        (cur as f64 / 1_000_000_000.0, total as f64 / 1_000_000_000.0, "GB")
    } else if total >= 1_000_000 {
        (cur as f64 / 1_000_000.0, total as f64 / 1_000_000.0, "MB")
    } else if total >= 1_000 {
        (cur as f64 / 1_000.0, total as f64 / 1_000.0, "KB")
    } else {
        (cur as f64, total as f64, "B")
    };
    let fmt = |v: f64| -> String {
        let s = format!("{:.1}", v);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    format!("{}/{} {}", fmt(cur_f), fmt(total_f), unit)
}

/// 将秒数格式化为 HH:MM:SS。
pub(crate) fn format_eta_hms(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{}:{:02}:{:02}", h, m, s)
}

/// 在进度条上设置 ETA（HH:MM:SS 格式），基于已用时间和当前进度估算。
pub(crate) fn set_progress_eta(bar: &indicatif::ProgressBar) {
    let pos = bar.position();
    let len = bar.length().unwrap_or(0);
    let elapsed = bar.elapsed().as_secs_f64();
    if len > 0 && pos > 0 && elapsed > 0.0 {
        let fraction = pos as f64 / len as f64;
        let eta_secs = (elapsed / fraction - elapsed) as u64;
        bar.set_message(format_eta_hms(eta_secs));
    } else {
        bar.set_message("--:--:--");
    }
}

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

/// 清理所有策略留下的临时文件（`{base}.s{src}_{strat}.tmp`、`{base}.strategy_N.tmp`、`BIT*.tmp` 等）。
/// 不会删除 `target_path` 自身。
fn cleanup_strategy_temp(target_path: &Path) {
    let parent = target_path.parent().unwrap_or(Path::new("."));
    let stem = target_path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let target_name = stem.clone();
    if stem.is_empty() {
        return;
    }
    // 剥离 .downloading 后缀得到基础文件名，用于匹配 s*_*.tmp 等策略临时文件。
    // 例如 target="foo.exe.downloading" → base="foo.exe"
    let base = stem.strip_suffix(".downloading").unwrap_or(&stem).to_string();
    if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            // 跳过目标文件自身
            if name == target_name {
                continue;
            }
            // 匹配：
            //   - 以 stem 或 base 开头的 .tmp / .downloading / .aria2 文件
            //   - *.parts.* 目录
            //   - BIT*.tmp（BITS 中途取消留下的临时文件）
            let starts_with_stem = name.starts_with(&stem);
            let starts_with_base = name.starts_with(&base);
            let is_tmp = name.ends_with(".tmp") || name.ends_with(".downloading") || name.ends_with(".aria2");
            let is_parts_dir = name.contains(".parts.");
            let is_bits_orphan = name.starts_with("BIT") && name.ends_with(".tmp");
            if is_tmp && (starts_with_stem || starts_with_base || is_bits_orphan) || is_parts_dir {
                let _ = if path.is_dir() {
                    std::fs::remove_dir_all(&path)
                } else {
                    std::fs::remove_file(&path)
                };
            }
        }
    }
}

/// 进度条上下文 — 将 bar 和名字打包，用于子函数更新进度。
#[derive(Clone)]
pub struct ProgressCtx {
    pub bar: indicatif::ProgressBar,
    pub name: String,
}

impl ProgressCtx {
    pub fn new(bar: indicatif::ProgressBar, name: &str) -> Self {
        Self { bar, name: name.to_string() }
    }
}

/// Ctrl+C 被按下时设为 true，各环节检查此标志后优雅退出。
static CTRL_C_PRESSED: AtomicBool = AtomicBool::new(false);

/// 安装 Ctrl+C 处理器（进程生命期仅安装一次）。
fn install_ctrlc_handler() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        ctrlc::set_handler(|| {
            CTRL_C_PRESSED.store(true, Ordering::Relaxed);
        })
        .expect("Ctrl+C 信号处理器安装失败");
    });
}

/// pip 风格下载：逐后端顺序尝试，同一后端的 URL 逐个重试。
///
/// 每个后端尝试时显示一个独立进度条，完成后输出一行摘要。
/// 当前端全部 URL 失败后才切换到下一个后端。
pub fn download_with_fallback(
    urls: &[String],
    target_path: &Path,
    config: &DownloadConfig,
) -> anyhow::Result<DownloadReport> {
    install_ctrlc_handler();
    CTRL_C_PRESSED.store(false, Ordering::Relaxed);
    let start = Instant::now();

    if urls.is_empty() {
        bail!("没有可用的下载地址");
    }

    // ── 确定后端列表：配置文件优先 ──
    let (backends, verify_enabled) = {
        let file_cfg = crate::config::DownloaderConfig::load();
        if !file_cfg.backends.is_empty() && config_file_exists() {
            (file_cfg.backends, file_cfg.verify)
        } else {
            (backends_from_config(config), config.verify)
        }
    };

    // 清理上次残留的临时文件
    let _ = std::fs::remove_file(target_path.with_extension("tmp"));

    // ── 进度条模板 ──
    let tracked_style = indicatif::ProgressStyle::default_bar()
        .template("    {bar:40.green/white} {prefix:.green} {decimal_bytes_per_sec:.red} {msg:.cyan}")
        .unwrap()
        .progress_chars("━━━");

    // ── 筛选可用后端 ──
    let available: Vec<Box<dyn DownloadBackend>> = backends.into_iter()
        .filter(|b| b.health_check())
        .collect();

    if available.is_empty() {
        bail!("没有可用的下载后端");
    }

    let mut last_error: Option<String> = None;

    // ── 逐后端顺序尝试 ──
    for backend in &available {
        if CTRL_C_PRESSED.load(Ordering::Relaxed) {
            bail!("用户取消 (Ctrl+C)");
        }

        let sname = backend.display_name();
        let tracked = backend.tracked();

        // 创建一个进度条（注册到 MultiProgress 确保渲染）
        let bar = progress().add(indicatif::ProgressBar::new(1));
        if tracked {
            bar.set_style(tracked_style.clone());
        }
        let ctx = ProgressCtx::new(bar, sname);
        let cancel = Cancel::new();

        eprintln!("    使用 {} ({})", yellow(sname), backend.thread_label());

        // 逐 URL 尝试
        for url in urls {
            if CTRL_C_PRESSED.load(Ordering::Relaxed) {
                bail!("用户取消 (Ctrl+C)");
            }

            let tmp_path = target_path.with_extension("tmp");
            let _ = std::fs::remove_file(&tmp_path);

            let result = backend.download(
                url,
                &tmp_path,
                &cancel,
                if tracked { Some(ctx.clone()) } else { None },
            );

            match result {
                Ok(()) => {
                    // ── 下载成功 ──
                    if tracked {
                        ctx.bar.finish();
                    }

                    // 执行文件校验
                    if verify_enabled && !verify_downloaded_file(&tmp_path) {
                        let _ = std::fs::remove_file(&tmp_path);
                        bail!("{}: 下载内容签名不匹配", sname);
                    }

                    // 移动到目标路径
                    let _ = std::fs::remove_file(target_path);
                    std::fs::rename(&tmp_path, target_path)
                        .context("重命名临时文件到目标路径失败")?;

                    let file_size = std::fs::metadata(target_path)
                        .map(|m| m.len()).unwrap_or(0);
                    let elapsed = start.elapsed();

                    eprintln!("    {} {}", yellow(sname), green("下载完成"));

                    return Ok(DownloadReport {
                        strategy_used: sname.to_string(),
                        total_bytes: file_size,
                        elapsed,
                        url: url.clone(),
                        target_path: target_path.to_path_buf(),
                    });
                }
                Err(e) => {
                    // 当前 URL 失败 → 清理临时文件，继续尝试下一地址
                    let _ = std::fs::remove_file(&tmp_path);
                    last_error = Some(format!("{}", e));
                }
            }
        }

        // 该后端所有 URL 均失败 → 冻结条，打印失败信息
        if tracked {
            ctx.bar.finish();
        }
        if let Some(ref err) = last_error {
            eprintln!("    {} {}: {}", yellow(sname), red("下载失败"), err);
        }
    }

    // 所有后端均失败
    if CTRL_C_PRESSED.load(Ordering::Relaxed) {
        bail!("用户取消 (Ctrl+C)");
    }
    bail!("下载失败: {}", last_error.as_deref().unwrap_or("未知错误"));
}

/// 检查配置文件是否存在。
fn config_file_exists() -> bool {
    crate::config::config_file_path().is_file()
}

/// 从旧的 `DownloadConfig` 转换后端列表（配置文件不存在时的降级）。
fn backends_from_config(config: &DownloadConfig) -> Vec<Box<dyn DownloadBackend>> {
    config.strategies.iter().map(|s| {
        let be: Box<dyn DownloadBackend> = match s {
            DownloadStrategy::RustRange { threads } => {
                Box::new(crate::backend::RustRangeBackend { threads: *threads, resume: config.resume })
            }
            DownloadStrategy::Aria2c => Box::new(crate::backend::Aria2cBackend),
            DownloadStrategy::Curl => Box::new(crate::backend::CurlBackend),
            DownloadStrategy::PowerShell => Box::new(crate::backend::PowerShellBackend),
            DownloadStrategy::PowerShellInvoke => Box::new(crate::backend::PowerShellInvokeBackend),
            DownloadStrategy::BitsTransfer => Box::new(crate::backend::BitsBackend),
            DownloadStrategy::Ureq { insecure: true, .. } => Box::new(crate::backend::UreqBackend::insecure()),
            DownloadStrategy::Ureq { .. } => Box::new(crate::backend::UreqBackend::normal()),
        };
        be
    }).collect()
}

/// 使用 ureq Agent 单线程下载到文件（含进度条 + cookie 挑战回退）。
pub(crate) fn download_with_ureq(
    url: &str,
    target_path: &Path,
    agent_cfg: &AgentConfig,
    cancel: &Cancel,
    external_pb: Option<ProgressCtx>,
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
                eprintln!("      Cookie 验证，重试中...");
                last_err = None;
                continue;
            }

            if attempt == 1 && !had_cookie {
                // 尝试剥离 tads 参数（JS 挑战的清理逻辑）
                if let Some(pos) = current_url.find("?tads") {
                    current_url = current_url[..pos].to_string();
                    eprintln!("      清理 URL 参数重试...");
                    last_err = None;
                    continue;
                }
                if let Some(pos) = current_url.find("&tads") {
                    current_url = current_url[..pos].to_string();
                    eprintln!("      清理 URL 参数重试...");
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

        let pb: Option<indicatif::ProgressBar> = external_pb.as_ref().map(|ctx| {
            if total > 0 {
                ctx.bar.set_length(total);
            }
            ctx.bar.clone()
        });

        loop {
            if cancel.is_cancelled() {
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
                if total > 0 {
                    pb.set_prefix(crate::download::format_decimal_progress(pb.position(), total));
                    crate::download::set_progress_eta(pb);
                }
            }
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

    eprintln!("    正在探测 {} 个地址的可达性（8 秒超时）...", total);

    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let probe_timeout = Duration::from_secs(8);

    for url in urls {
        let tx = tx.clone();
        let url = url.clone();
        thread::spawn(move || {
            // 使用与下载一致的 Chrome120 UA，避免 CDN 屏蔽非浏览器请求
            let agent = ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_secs(5))
                .timeout_read(Duration::from_secs(5))
                .user_agent(Fingerprint::Chrome120.user_agent())
                .build();

            match agent.head(&url).call() {
                // 放宽过滤：只要 status < 500 即视为可达，
                // 去除 Content-Type 检查，避免 CDN 返回 text/html 时误判
                Ok(resp) if resp.status() < 500 => {
                    let _ = tx.send(url);
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
    let (immediate, deferred) = if alive.is_empty() {
        eprintln!("    地址探测无响应（8 秒超时），全部地址参与下载");
        (expanded, vec![])
    } else {
        let deferred: Vec<String> = expanded
            .iter()
            .filter(|u| !alive.contains(u))
            .cloned()
            .collect();
        (alive, deferred)
    };

    // ── Phase 2: 并行下载 ──
    // 先用探测可达的地址，全部失败且有未探测地址时降级重试
    download_with_fallback(&immediate, target_path, config).or_else(|first_err| {
        if deferred.is_empty() {
            return Err(first_err);
        }
        eprintln!("    初始地址均不可用，使用 {} 个备选地址重试...", deferred.len());
        cleanup_strategy_temp(target_path);
        download_with_fallback(&deferred, target_path, config)
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_decimal_progress() {
        assert_eq!(format_decimal_progress(0, 0), "0/0 B");
        assert_eq!(format_decimal_progress(500, 1000), "0.5/1 KB");
        assert_eq!(format_decimal_progress(1_500_000, 2_000_000), "1.5/2 MB");
        assert_eq!(format_decimal_progress(1_500_000_000, 2_000_000_000), "1.5/2 GB");
        assert_eq!(format_decimal_progress(1_000_000, 2_000_000), "1/2 MB");
    }

    #[test]
    fn test_format_eta_hms() {
        assert_eq!(format_eta_hms(0), "0:00:00");
        assert_eq!(format_eta_hms(59), "0:00:59");
        assert_eq!(format_eta_hms(60), "0:01:00");
        assert_eq!(format_eta_hms(3661), "1:01:01");
        assert_eq!(format_eta_hms(86399), "23:59:59");
    }

    #[test]
    fn test_expand_github_urls() {
        let urls = vec!["https://github.com/user/repo/releases/download/v1/file.zip".to_string()];
        let expanded = expand_github_urls(&urls);
        assert_eq!(expanded.len(), 2);
        assert!(expanded[0].contains("ghproxy.net"));
        assert_eq!(&expanded[1], &urls[0]);

        // Non-github URL should not be expanded
        let urls2 = vec!["https://example.com/file.zip".to_string()];
        let expanded2 = expand_github_urls(&urls2);
        assert_eq!(expanded2.len(), 1);

        // Already proxied URL should not be expanded
        let urls3 = vec!["https://ghproxy.net/https://github.com/user/repo".to_string()];
        let expanded3 = expand_github_urls(&urls3);
        assert_eq!(expanded3.len(), 1);
    }
}
