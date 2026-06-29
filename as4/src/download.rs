use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use color::*;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::software;

pub fn optimal_concurrency(software_count: usize) -> (usize, u8) {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    match software_count {
        0 => (0, 0),
        1 => (1, (cpus as u8 * 4).min(32).max(4)),
        n if n <= cpus.max(4) / 2 => (n, 8),
        _ => (cpus.max(4) / 2, 4),
    }
}

#[derive(Clone)]
pub struct DownloadResult {
    pub name: String,
    pub version: String,
    pub file_name: String,
    pub file_path: PathBuf,
    pub success: bool,
    pub error: Option<String>,
}

pub fn download_all(
    targets: Vec<(String, String, software::VersionEntry, String)>,
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

    for (name, version, entry, url_type) in targets {
        let active = active.clone();
        let mp = mp.clone();
        let results = results.clone();

        handles.push(std::thread::spawn(move || {
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

            let result = download_one(&name, &version, &entry, &url_type, &mp, range_threads);

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

fn download_one(
    name: &str,
    version: &str,
    entry: &software::VersionEntry,
    url_type: &str,
    mp: &MultiProgress,
    range_threads: u8,
) -> DownloadResult {
    let empty = vec![];
    let urls = entry.urls.get(url_type).unwrap_or(&empty);
    if urls.is_empty() {
        return DownloadResult {
            name: name.to_string(),
            version: version.to_string(),
            file_name: String::new(),
            file_path: PathBuf::new(),
            success: false,
            error: Some(format!("未找到 {} 类型的下载地址", url_type)),
        };
    }

    let url = &urls[0];
    let file_name = extract_filename(url, name, version);
    let target = crate::paths::as_dir().join(&file_name);

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
                    file_path: target,
                    success: true,
                    error: None,
                };
            }
        }
    }

    let bar = mp.add(ProgressBar::new(1));
    bar.set_style(
        ProgressStyle::default_bar()
            .template("{prefix:12} [{bar:20.cyan/blue}] {bytes}/{total_bytes} {bytes_per_sec} {eta}")
            .unwrap()
            .progress_chars("▓▓░"),
    );
    bar.set_prefix(truncate_name(name, 12));
    bar.set_message("等待中...");

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
    match net::download::download_with_url_fallback(name, urls, &target, &config) {
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
                file_path: target,
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
                file_path: target,
                success: false,
                error: Some(e.to_string()),
            }
        }
    }
}

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

fn truncate_name(name: &str, max: usize) -> String {
    if name.display_width() <= max {
        name.to_string()
    } else {
        format!("{}..", &name[..max.saturating_sub(2)])
    }
}

fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let bytes = bytes as f64;
    if bytes <= 0.0 { return "0 B".into(); }
    let unit = (bytes.log10() / 3.0) as usize;
    let unit = unit.min(UNITS.len() - 1);
    let value = bytes / 1024u64.pow(unit as u32) as f64;
    format!("{:.1} {}", value, UNITS[unit])
}