use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use color::*;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::software;

// ── 智能并发计算 ──────────────────────────────────

/// 根据软件数量和 CPU 核心数计算最佳并发参数
///
/// 返回 (并发下载数, 单文件分片线程数)
pub fn optimal_concurrency(software_count: usize) -> (usize, u8) {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    match software_count {
        0 => (0, 0),
        1 => (1, (cpus as u8 * 4).min(32).max(4)),       // 单文件 → 全速分片
        n if n <= cpus.max(4) / 2 => (n, 8),              // 少量 → 并行 + 适中分片
        _ => (cpus.max(4) / 2, 4),                         // 大量 → 限制并发，减少分片
    }
}

// ── 下载管理 ──────────────────────────────────────

/// 下载结果
#[derive(Clone)]
#[allow(dead_code)]
pub struct DownloadResult {
    pub name: String,
    pub version: String,
    pub file_name: String,
    pub success: bool,
    pub error: Option<String>,
}

/// 并发下载多个软件
pub fn download_all(
    targets: Vec<(String, String, software::VersionEntry)>,
) -> anyhow::Result<Vec<DownloadResult>> {
    let count = targets.len();
    let (max_concurrent, range_threads) = optimal_concurrency(count);

    let active = Arc::new(Mutex::new(0usize));
    let mp = Arc::new(MultiProgress::new());
    let results = Arc::new(Mutex::new(Vec::new()));

    println!();
    println!("  {} 个文件, {} 个并发, 每个 {} 线程分片",
        bold_cyan(&count.to_string()),
        bold_cyan(&max_concurrent.to_string()),
        bold_cyan(&range_threads.to_string()),
    );
    println!();

    let mut handles = Vec::new();

    for (name, version, entry) in targets {
        let active = active.clone();
        let mp = mp.clone();
        let results = results.clone();

        handles.push(std::thread::spawn(move || {
            // 等待并发槽位
            loop {
                {
                    let mut a = active.lock().unwrap();
                    if *a < max_concurrent {
                        *a += 1;
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }

            let result = download_one(&name, &version, &entry, &mp, range_threads);

            {
                let mut a = active.lock().unwrap();
                *a = a.saturating_sub(1);
            }

            results.lock().unwrap().push(result);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    mp.clear().ok();

    let final_results = results.lock().unwrap().clone();
    let success_count = final_results.iter().filter(|r| r.success).count();
    let fail_count = final_results.iter().filter(|r| !r.success).count();

    println!();
    println!("  {} {} / {} 个文件下载完成",
        bold_green("完成"),
        bold_cyan(&success_count.to_string()),
        bold_cyan(&count.to_string()),
    );
    if fail_count > 0 {
        for r in &final_results {
            if let Some(ref e) = r.error {
                println!("  {} {}: {}", yellow("失败"), bold_cyan(&r.name), e);
            }
        }
    }
    println!();

    Ok(final_results)
}

// ── 单文件下载 ────────────────────────────────────

fn download_one(
    name: &str,
    version: &str,
    entry: &software::VersionEntry,
    mp: &MultiProgress,
    range_threads: u8,
) -> DownloadResult {
    let as_dir = as_dir();
    let url = &entry.urls[0];
    let file_name = extract_filename(url, name, version);
    let target = as_dir.join(&file_name);

    // 检查文件已存在
    if target.exists() {
        let msg = format!("  {} 已存在 ({}), 覆盖? [y/N] ",
            file_name, format_size(file_size(&target)));
        print!("{}", msg);
        std::io::stdout().flush().ok();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        match input.trim().to_lowercase().as_str() {
            "y" | "yes" => {
                std::fs::remove_file(&target).ok();
            }
            _ => {
                return DownloadResult {
                    name: name.to_string(),
                    version: version.to_string(),
                    file_name: file_name.clone(),
                    success: true,
                    error: None,
                };
            }
        }
    }

    // 创建进度条
    let bar = mp.add(ProgressBar::new(1));
    bar.set_style(
        ProgressStyle::default_bar()
            .template("{prefix:12} [{bar:20.cyan/blue}] {bytes}/{total_bytes} {bytes_per_sec} {eta}")
            .unwrap()
            .progress_chars("▓▓░"),
    );
    bar.set_prefix(truncate_name(name, 12));
    bar.set_message("等待中...");

    // 构建下载策略
    use net::agent::Fingerprint;
    use net::download::{DownloadConfig, DownloadStrategy};
    let config = DownloadConfig {
        strategies: vec![
            DownloadStrategy::RustRange { threads: range_threads },
            DownloadStrategy::Ureq { fingerprint: Fingerprint::Chrome120, insecure: false },
            DownloadStrategy::Ureq { fingerprint: Fingerprint::Chrome120, insecure: true },
            DownloadStrategy::PowerShell,
            DownloadStrategy::Curl,
        ],
        ..Default::default()
    };

    let start = std::time::Instant::now();
    match net::download::download_with_url_fallback(name, &entry.urls, &target, &config) {
        Ok(report) => {
            let elapsed = start.elapsed();
            bar.finish_with_message(format!("完成 ({})", format_size(report.total_bytes)));
            println!("  {} 下载完成: {} ({} 用时 {:.1}s)",
                bold_green("✓"),
                bold_cyan(&file_name),
                format_size(report.total_bytes),
                elapsed.as_secs_f64(),
            );
            DownloadResult {
                name: name.to_string(),
                version: version.to_string(),
                file_name: file_name.clone(),
                success: true,
                error: None,
            }
        }
        Err(e) => {
            bar.finish_with_message("失败");
            DownloadResult {
                name: name.to_string(),
                version: version.to_string(),
                file_name,
                success: false,
                error: Some(e.to_string()),
            }
        }
    }
}

// ── 辅助函数 ──────────────────────────────────────

/// 获取 as.exe 所在目录
pub fn as_dir() -> PathBuf {
    let exe = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("as.exe"));
    exe.parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// 从 URL 提取文件名
fn extract_filename(url: &str, name: &str, version: &str) -> String {
    let last = url.split('/')
        .filter(|s| !s.is_empty())
        .last()
        .unwrap_or("");
    let clean = last.split('?').next().unwrap_or(last);
    if clean.contains('.') && !clean.ends_with('.') {
        clean.to_string()
    } else {
        let ext = if url.contains(".zip") || url.contains(".7z") { "zip" } else { "exe" };
        format!("{}-{}.{}", name, version, ext)
    }
}

/// 截断名字到指定宽度
fn truncate_name(name: &str, max: usize) -> String {
    if name.display_width() <= max {
        name.to_string()
    } else {
        format!("{}..", &name[..max.saturating_sub(2)])
    }
}

/// 文件大小显示
fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// 格式化字节数
fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let bytes = bytes as f64;
    if bytes <= 0.0 { return "0 B".into(); }
    let unit = (bytes.log10() / 3.0) as usize;
    let unit = unit.min(UNITS.len() - 1);
    let value = bytes / 1024u64.pow(unit as u32) as f64;
    format!("{:.1} {}", value, UNITS[unit])
}
