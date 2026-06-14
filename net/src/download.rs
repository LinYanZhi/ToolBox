use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::bail;
use indicatif::MultiProgress;

use crate::backend::DownloadBackend;

use crate::agent::{AgentConfig, Fingerprint};
use crate::verify::verify_downloaded_file;

const CHUNK: usize = 64 * 1024;

/// 最大并行下载组合数，防止 URL × 策略组合爆炸。
const MAX_CONCURRENT_COMBOS: usize = 16;

/// 并发许可池 — 限制同时运行的下载线程数量。
///
/// 每个线程在开始下载前 `try_acquire()` 等待许可，下载完成后自动归还。
/// 线程在等待期间每 500ms 检查一次取消状态，避免被阻塞后无法响应取消。
struct PermitPool {
    inner: Arc<(Mutex<usize>, Condvar)>,
    max: usize,
}

impl PermitPool {
    fn new(max: usize) -> Self {
        Self { inner: Arc::new((Mutex::new(0usize), Condvar::new())), max }
    }

    /// 尝试获取一个许可，最长等待 `timeout` 时间。
    /// 返回 `None` 表示超时（调用方应检查取消状态后重试）。
    fn try_acquire(&self, timeout: Duration) -> Option<PermitGuard> {
        let (lock, cvar) = &*self.inner;
        let mut count = lock.lock().unwrap();
        let deadline = Instant::now() + timeout;
        while *count >= self.max {
            let now = Instant::now();
            if now >= deadline {
                return None;
            }
            count = cvar.wait_timeout(count, deadline - now).unwrap().0;
        }
        *count += 1;
        Some(PermitGuard { inner: Arc::clone(&self.inner) })
    }
}

struct PermitGuard {
    inner: Arc<(Mutex<usize>, Condvar)>,
}

impl Drop for PermitGuard {
    fn drop(&mut self) {
        let (lock, cvar) = &*self.inner;
        let mut count = lock.lock().unwrap();
        *count = count.saturating_sub(1);
        cvar.notify_one();
    }
}

// ── ANSI 颜色常量（用于进度条表格） ──
const C_RESET: &str = "\x1b[0m";
const C_BOLD: &str = "\x1b[1m";
const C_RED: &str = "\x1b[31m";
const C_GREEN: &str = "\x1b[32m";
const C_YELLOW: &str = "\x1b[33m";
const C_MAGENTA: &str = "\x1b[35m";
const C_CYAN: &str = "\x1b[36m";
const C_GREY: &str = "\x1b[90m";

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

/// 终端显示宽度：CJK 字符占 2 列，其余占 1 列。
fn display_width(s: &str) -> usize {
    s.chars().map(|c| {
        if c >= '\u{4e00}' && c <= '\u{9fff}'
            || c >= '\u{3400}' && c <= '\u{4dbf}'
            || c >= '\u{ff00}' && c <= '\u{ffef}'
        { 2 } else { 1 }
    }).sum()
}

/// 将字符串填充到指定的终端显示宽度（不是 Rust 字符数）。
fn pad_display(s: &str, width: usize) -> String {
    let w = display_width(s);
    if w < width {
        let mut r = s.to_string();
        r.push_str(&" ".repeat(width - w));
        r
    } else {
        s.to_string()
    }
}

/// 进度条上下文 — 将 (col1, tlabel) 与 bar 打包，用于子函数更新 status。
#[derive(Clone)]
pub struct ProgressCtx {
    pub bar: indicatif::ProgressBar,
    col1: String,
    tlabel: std::cell::Cell<&'static str>,
}

impl ProgressCtx {
    pub fn new(bar: indicatif::ProgressBar, col1: &str, tlabel: &'static str) -> Self {
        Self { bar, col1: col1.to_string(), tlabel: std::cell::Cell::new(tlabel) }
    }
    /// 更新状态列（如 "下载中" / "✓ 完成"），自动补齐对齐 + ANSI 颜色。
    /// 速度信息通过模板的 {bytes_per_sec} 展示在进度条后方，不从 msg 加。
    pub fn set_status(&self, status: &str) {
        let base = build_msg(&self.col1, self.tlabel.get(), status);
        self.bar.set_message(base);
    }
    /// 更新线程标签（如 "多线程" → "单线程"），再调 set_status 即可刷新显示。
    pub fn set_thread_label(&self, label: &'static str) {
        self.tlabel.set(label);
    }
}

fn colored_thread(t: &str) -> String {
    match t {
        "多线程" => format!("{}{}{}", C_CYAN, pad_display(t, 8), C_RESET),
        "单线程" => format!("{}{}{}", C_MAGENTA, pad_display(t, 8), C_RESET),
        _ => pad_display(t, 8),
    }
}

fn colored_status(s: &str) -> String {
    let trimmed = s.trim_end();
    let padded = pad_display(trimmed, 10);
    match trimmed {
        "等待中" | "待命中" => format!("{}{}{}", C_GREY, padded, C_RESET),
        "运行中" | "连接中" => format!("{}{}{}", C_YELLOW, padded, C_RESET),
        "下载中" => format!("{}{}{}", C_BOLD, padded, C_RESET),
        _ if trimmed.starts_with('✓') => format!("{}{}{}", C_GREEN, padded, C_RESET),
        _ if trimmed.starts_with('✗') => format!("{}{}{}", C_RED, padded, C_RESET),
        _ => padded,
    }
}

/// 构建 5 列进度条消息。
///   列 1：工具名（14 列，pad）
///   列 2：线程（8 列，彩色）
///   列 3：状态（10 列，彩色）
///   ── 以上共 32 列 ──
///   列 4：进度条（模板中渲染）
///   列 5：进度数值（模板中渲染）
pub(crate) fn build_msg(col1: &str, thread: &str, status: &str) -> String {
    let col1 = pad_display(col1, 14);
    let col2 = colored_thread(thread);
    let col3 = colored_status(status);
    format!("{}{}{}", col1, col2, col3)
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

/// 并行下载：逐策略顺序回退，同策略地址并行。
///
/// 设计原则：避免策略间互相抢带宽。
/// 先尝试最快的策略（RustRange/aria2c/ureq）并行下载所有 URL，
/// 所有 URL 都用同一策略失败后才切换到下一个策略。
/// 临时文件命名 `{target}.s{strat_idx}_{url_idx}.tmp`。
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
    let (backends, max_concurrent, verify_enabled) = {
        let file_cfg = crate::config::DownloaderConfig::load();
        // 如果配置文件存在且解析成功（backends 非空），则用它
        if !file_cfg.backends.is_empty() && config_file_exists() {
            let names: Vec<&str> = file_cfg.backends.iter().map(|b| b.id()).collect();
            let backends: Vec<Box<dyn DownloadBackend>> = file_cfg.backends.into_iter().collect();
            eprintln!("    {}>>{} {} 个地址 × {} 个后端（配置: {}）",
                C_GREEN, C_RESET, urls.len(), backends.len(), names.join(", "));
            (backends, file_cfg.max_concurrent, file_cfg.verify)
        } else {
            // 配置文件不存在 → 从旧 config 参数转换
            let backends = backends_from_config(config);
            eprintln!("    {}>>{} {} 个地址 × {} 个后端（默认）",
                C_GREEN, C_RESET, urls.len(), backends.len());
            (backends, MAX_CONCURRENT_COMBOS, config.verify)
        }
    };

    // 开始前清理上一次残留的临时文件
    cleanup_strategy_temp(target_path);

    // ── 进度条模板 ──
    let tracked_style = indicatif::ProgressStyle::default_bar()
        .template("{msg} [{bar:18.green/white}] {bytes:.green}/{total_bytes:.green} ({bytes_per_sec:.green}, +{elapsed_precise:.cyan} / -{eta:.yellow})")
        .unwrap()
        .progress_chars("━━━");
    let untracked_style = indicatif::ProgressStyle::default_bar()
        .template("{msg}")
        .unwrap();

    let mut last_error: Option<String> = None;
    let backends = Arc::new(backends);

    // ── 筛选可用后端 ──
    let available: Vec<(usize, &Box<dyn DownloadBackend>)> = (0..backends.len())
        .filter_map(|i| {
            if backends[i].health_check() { Some((i, &backends[i])) } else { None }
        })
        .collect();

    if available.is_empty() {
        bail!("没有可用的下载后端");
    }

    eprintln!();

    // ── 预分配所有进度条（固定行位置，永不移动） ──
    // bar_grid[abi][ui] = 第 abi 个可用后端的第 ui 个 URL 的进度条
    let mut bar_grid: Vec<Vec<ProgressCtx>> = Vec::with_capacity(available.len());
    for (_, be) in &available {
        let t = be.tracked();
        let mut row = Vec::with_capacity(urls.len());
        for _ in 0..urls.len() {
            let bar = progress().add(indicatif::ProgressBar::new(1));
            if t { bar.set_style(tracked_style.clone()); }
            else { bar.set_style(untracked_style.clone()); }
            let ctx = ProgressCtx::new(bar, be.display_name(), be.thread_label());
            ctx.set_status("等待中");
            row.push(ctx);
        }
        bar_grid.push(row);
    }

    // ── 逐后端顺序尝试（每行位置固定） ──
    for abi in 0..available.len() {
        let (orig_idx, _be) = &available[abi];
        let backend = &backends[*orig_idx];
        let sname = backend.display_name();
        let tracked = backend.tracked();

        // 激活当前行
        let bars: Vec<ProgressCtx> = bar_grid[abi].iter().map(|b| b.clone()).collect();
        for b in &bars { b.set_status("运行中"); }

        let someone_won = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel::<(String, u64, PathBuf)>();
        let pool = Arc::new(PermitPool::new(max_concurrent));
        let mut cancel_handles: Vec<Cancel> = Vec::with_capacity(urls.len());
        let completed_files: Arc<Mutex<Vec<PathBuf>>> = Arc::new(Mutex::new(Vec::new()));

        // ── 启动每个 URL 的下载线程（PermitPool 限流） ──
        for (ui, url) in urls.iter().enumerate() {
            let tmp_path = target_path.with_extension(format!("s{}_{}.tmp", *orig_idx, ui));
            let cancel = Cancel::new();
            cancel_handles.push(cancel.clone());

            let url = url.clone();
            let bar = bars[ui].clone();
            let tx = tx.clone();
            let someone_won = Arc::clone(&someone_won);
            let completed_files = Arc::clone(&completed_files);
            let pool = Arc::clone(&pool);
            let sname = sname.to_string();
            let backends_arc = Arc::clone(&backends);
            let si = *orig_idx;
            let tmp_path_for_completed = tmp_path.clone();

            std::thread::spawn(move || {
                loop {
                    if someone_won.load(Ordering::Relaxed) {
                        if tmp_path_for_completed.is_file() {
                            let file_size = std::fs::metadata(&tmp_path_for_completed).map(|m| m.len()).unwrap_or(0);
                            if file_size > 0 {
                                if let Ok(mut cf) = completed_files.lock() {
                                    cf.push(tmp_path_for_completed.clone());
                                }
                            }
                        }
                        ProgressCtx::new(bar.bar.clone(), &sname, backends_arc[si].thread_label())
                            .set_status("✗ 已取消");
                        let _ = std::fs::remove_file(&tmp_path_for_completed);
                        return;
                    }
                    if let Some(_permit) = pool.try_acquire(Duration::from_millis(500)) {
                        break;
                    }
                }
                if someone_won.load(Ordering::Relaxed) {
                    ProgressCtx::new(bar.bar.clone(), &sname, backends_arc[si].thread_label())
                        .set_status("✗ 已取消");
                    let _ = std::fs::remove_file(&tmp_path_for_completed);
                    return;
                }

                let result = backends_arc[si].download(&url, &tmp_path, &cancel,
                    if tracked { Some(bar.clone()) } else { None });

                match result {
                    Ok(_) => {
                        if someone_won.swap(true, Ordering::Relaxed) {
                            if tmp_path.is_file() {
                                let file_size = std::fs::metadata(&tmp_path).map(|m| m.len()).unwrap_or(0);
                                if file_size > 0 {
                                    if let Ok(mut cf) = completed_files.lock() {
                                        cf.push(tmp_path.clone());
                                    }
                                }
                            }
                            ProgressCtx::new(bar.bar.clone(), &sname, backends_arc[si].thread_label())
                                .set_status("✗ 已取消");
                            return;
                        }
                        let file_size = std::fs::metadata(&tmp_path).map(|m| m.len()).unwrap_or(0);
                        ProgressCtx::new(bar.bar.clone(), &sname, backends_arc[si].thread_label())
                            .set_status("✓ 完成");
                        let _ = tx.send((url.clone(), file_size, tmp_path.clone()));
                    }
                    Err(e) => {
                        let emsg = format!("{}", e);
                        let short = if emsg.chars().count() > 6 {
                            let truncated: String = emsg.chars().take(6).collect();
                            format!("{}…", truncated)
                        } else { emsg };
                        if someone_won.load(Ordering::Relaxed) {
                            if tmp_path.is_file() {
                                let file_size = std::fs::metadata(&tmp_path).map(|m| m.len()).unwrap_or(0);
                                if file_size > 0 {
                                    if let Ok(mut cf) = completed_files.lock() {
                                        cf.push(tmp_path.clone());
                                    }
                                }
                            }
                            ProgressCtx::new(bar.bar.clone(), &sname, backends_arc[si].thread_label())
                                .set_status("✗ 已取消");
                        } else {
                            ProgressCtx::new(bar.bar.clone(), &sname, backends_arc[si].thread_label())
                                .set_status(&format!("✗ {}", short));
                        }
                        let _ = std::fs::remove_file(&tmp_path);
                    }
                }
            });
        }

        // 监控循环：等待第一个成功，或全部失败
        let round_start = std::time::Instant::now();
        let grace_period = Duration::from_secs(12);
        let round_result = loop {
            match rx.recv_timeout(Duration::from_secs(2)) {
                Ok((url, file_size, tmp_path)) => break Ok((url, file_size, tmp_path)),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // 刷新活动行的速度显示
                    for b in &bars {
                        let status = b.bar.message();
                        if status.contains("运行中") || status.contains("下载中") || status.contains("连接中") {
                            b.set_status("下载中");
                        }
                    }
                    if CTRL_C_PRESSED.load(Ordering::Relaxed) {
                        for c in &cancel_handles { c.cancel(); }
                        break Err(anyhow::anyhow!("用户取消 (Ctrl+C)"));
                    }
                    if round_start.elapsed() < grace_period { continue; }
                    let all_idle = cancel_handles.iter().all(|c| c.is_cancelled() || c.last_progress_ms() == 0);
                    if all_idle {
                        let completed = completed_files.lock().unwrap_or_else(|e| e.into_inner());
                        if let Some(fp) = completed.first().filter(|p| p.is_file()) {
                            let file_size = std::fs::metadata(fp).map(|m| m.len()).unwrap_or(0);
                            if file_size > 0 {
                                let path = fp.clone();
                                break Ok((urls[0].clone(), file_size, path));
                            }
                        }
                        break Err(anyhow::anyhow!("{} 所有地址均失败", sname));
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    let completed = completed_files.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(fp) = completed.first().filter(|p| p.is_file()) {
                        let file_size = std::fs::metadata(fp).map(|m| m.len()).unwrap_or(0);
                        if file_size > 0 {
                            let path = fp.clone();
                            break Ok((urls[0].clone(), file_size, path));
                        }
                    }
                    break Err(anyhow::anyhow!("所有下载线程已退出"));
                }
            }
        };

        // 取消剩余线程
        for c in &cancel_handles { c.cancel(); }
        thread::sleep(Duration::from_millis(500));

        // 处理结果（错误显示在对应行，不输出 eprintln! 破坏布局）
        match round_result {
            Ok((url, _file_size, tmp_path)) => {
                // 标记当前行为成功
                for b in &bars {
                    if tracked { b.bar.finish(); }
                }
                // 标记后续后端的行（如果有）为已跳过
                for later_abi in (abi + 1)..bar_grid.len() {
                    for b in &bar_grid[later_abi] {
                        let msg = b.bar.message();
                        if !msg.contains('✓') && !msg.contains('✗') {
                            b.set_status("✗ 已跳过");
                            if tracked { b.bar.abandon(); }
                        }
                    }
                }
                let _ = std::fs::remove_file(target_path);
                if std::fs::rename(&tmp_path, target_path).is_err() {
                    cleanup_strategy_temp(target_path);
                    bail!("胜出后端的临时文件不存在");
                }
                if verify_enabled && !verify_downloaded_file(target_path) {
                    let _ = std::fs::remove_file(target_path);
                    cleanup_strategy_temp(target_path);
                    bail!("{}: 下载内容签名不匹配", sname);
                }
                cleanup_strategy_temp(target_path);
                let file_size = std::fs::metadata(target_path).map(|m| m.len()).unwrap_or(0);
                let elapsed = start.elapsed();
                return Ok(DownloadReport {
                    strategy_used: sname.to_string(),
                    total_bytes: file_size, elapsed, url,
                    target_path: target_path.to_path_buf(),
                });
            }
            Err(e) => {
                if CTRL_C_PRESSED.load(Ordering::Relaxed) {
                    let _ = std::fs::remove_file(target_path);
                    cleanup_strategy_temp(target_path);
                    bail!("{}", e);
                }
                // 失败信息显示在当前行，不动固定布局
                for b in &bars {
                    if tracked { b.bar.abandon(); }
                }
                cleanup_strategy_temp(target_path);
                last_error = Some(format!("{}", e));
            }
        }
    }

    // 所有策略均失败
    let _ = std::fs::remove_file(target_path);
    cleanup_strategy_temp(target_path);
    if CTRL_C_PRESSED.load(Ordering::Relaxed) {
        bail!("用户取消 (Ctrl+C)");
    }
    bail!("下载失败: {}", last_error.as_deref().unwrap_or("未知错误"));
}

/// 检查配置文件是否存在。
fn config_file_exists() -> bool {
    let local = std::env::var("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let path = local.join("aminos").join("config").join("download.toml");
    path.is_file()
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

        let pb: Option<indicatif::ProgressBar> = external_pb.as_ref().map(|ctx| {
            if total > 0 {
                ctx.bar.set_length(total);
            }
            ctx.bar.clone()
        });
        let mut first_byte = true;

        loop {
            if cancel.is_cancelled() {
                if let Some(ref ctx) = external_pb {
                    ctx.set_status("✗ 已取消");
                }
                let _ = std::fs::remove_file(target_path);
                return Err(anyhow::anyhow!("已取消"));
            }
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            if first_byte {
                first_byte = false;
                if let Some(ref ctx) = external_pb {
                    ctx.set_status("下载中");
                }
            }
            file.write_all(&buf[..n])?;
            cancel.mark_progress();
            if let Some(ref pb) = pb {
                pb.inc(n as u64);
            }
        }

        if let Some(ref ctx) = external_pb {
            ctx.set_status("✓ 完成");
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

    eprintln!("    {}>>{} 并行探测 {} 个地址（8s 封顶）", C_GREEN, C_RESET, total);

    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let probe_timeout = Duration::from_secs(8);

    for url in urls {
        let tx = tx.clone();
        let url = url.clone();
        std::thread::spawn(move || {
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
        eprintln!("    ⚠ 全部探测无响应（8s 超时），降级为全部地址参与");
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
        eprintln!("    ⚠ 初始地址全部失败，降级到 {} 个未探测地址重试...", deferred.len());
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
