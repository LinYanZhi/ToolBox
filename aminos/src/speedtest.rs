use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use crate::downloader::{self, display_width, format_size, pad, expand_github_urls};

use crate::software::{self, SoftwareDef};

pub fn speedtest(names: &[String], per_software: bool) -> anyhow::Result<()> {
    let start = Instant::now();

    let defs: Vec<SoftwareDef> = if names.is_empty() {
        software::list_software_defs()?
    } else {
        let mut v = Vec::new();
        for n in names {
            match software::read_software_def(n) {
                Ok(sd) => v.push(sd),
                Err(_e) => eprintln!("错误: 未找到软件 '{}' 的定义", n),
            }
        }
        v
    };

    if defs.is_empty() {
        println!("没有可用的软件定义。");
        return Ok(());
    }

    // Collect all URLs (auto-expand GitHub mirrors)
    let mut entries: Vec<(String, String, String)> = Vec::new();
    for sd in &defs {
        let display = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };
        for (vk, vi) in &sd.versions {
            let mut seen: HashMap<String, bool> = HashMap::new();
            let expanded = expand_github_urls(&vi.urls);
            for u in expanded {
                if !seen.contains_key(&u) {
                    seen.insert(u.clone(), true);
                    entries.push((display.to_string(), vk.clone(), u));
                }
            }
        }
    }

    let total = entries.len();
    if total == 0 {
        println!("没有可测的下载地址。");
        return Ok(());
    }

    // Pre-calculate max widths
    let max_name_w = entries.iter()
        .map(|(d, _, _)| display_width(d))
        .max()
        .unwrap_or(6)
        .max(6);

    let max_idx_w = total.to_string().len();
    let speed_w: usize = 12;

    println!("\n\x1b[90m共 {} 个下载源，正在并发测速...\x1b[0m\n", total);

    // Concurrent speed test — chunked to limit concurrency (match Python's max_workers=12)
    let results = Arc::new(Mutex::new(Vec::new()));
    let counter = Arc::new(Mutex::new(0u32));
    let print_lock = Arc::new(Mutex::new(()));

    const MAX_WORKERS: usize = 12;
    for chunk in entries.chunks(MAX_WORKERS) {
        let mut handles = Vec::new();
        for (display, version, url) in chunk {
            let display = display.clone();
            let version = version.clone();
            let url = url.clone();
            let counter = Arc::clone(&counter);
            let results = Arc::clone(&results);
            let print_lock = Arc::clone(&print_lock);
            handles.push(thread::spawn(move || {
                let speed = downloader::measure_speed(&url, 10);

                let mut idx = counter.lock().unwrap();
                *idx += 1;
                let current = *idx;
                drop(idx);

                // Print with lock for consistent ordering
                let _guard = print_lock.lock().unwrap();
                let (plain, color) = match speed {
                    Some(s) => (format_size(s * 1024.0) + "/s", "\x1b[32m"),
                    None => ("不可用".to_string(), "\x1b[33m"),
                };
                let marker = format!("{}{}{}", color, pad(&plain, speed_w), "\x1b[0m");

                let idx_str = format!("{:0>w$}", current, w = max_idx_w);
                let prefix = format!("  [{}/{}] {}", idx_str, total, pad(&display, max_name_w + 1));
                println!("{}{}  {}", prefix, pad(&marker, speed_w), url);

                results.lock().unwrap().push((display, version, url, speed));
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
    }

    // Summary table
    let elapsed = start.elapsed().as_secs_f64();
    let results = results.lock().unwrap();

    println!("\n\x1b[32m{}\x1b[0m", "═".repeat(70));

    if per_software {
        // ── 以软件为单位统计 ──
        let mut by_sw: HashMap<String, (String, Vec<&(String, String, String, Option<f64>)>)> = HashMap::new();
        for r in results.iter() {
            by_sw.entry(r.0.clone())
                .or_insert_with(|| (r.0.clone(), Vec::new()))
                .1.push(r);
        }

        let mut summary: Vec<(String, String, bool, Option<f64>)> = Vec::new();
        for (_key, (_disp, urls)) in &by_sw {
            let working: Vec<_> = urls.iter().filter(|r| r.3.is_some()).collect();
            let best = working.iter().map(|r| r.3.unwrap()).fold(None::<f64>, |acc, s| {
                Some(acc.map_or(s, |a| a.max(s)))
            });
            let versions: Vec<&str> = urls.iter().map(|r| r.1.as_str()).collect();
            // Deduplicate versions
            let mut vset = std::collections::HashSet::new();
            for v in &versions { vset.insert(*v); }
            let mut unique_vers: Vec<&str> = vset.into_iter().collect();
            unique_vers.sort();
            let ver_str = if unique_vers.len() > 1 {
                unique_vers.join(",")
            } else {
                unique_vers.first().copied().unwrap_or("").to_string()
            };
            summary.push((_disp.clone(), ver_str, !working.is_empty(), best));
        }

        summary.sort_by(|a, b| a.0.cmp(&b.0));

        let name_w = summary.iter().map(|(n, _, _, _)| display_width(n)).max().unwrap_or(4).max(4).min(20);
        let ver_w = summary.iter().map(|(_, v, _, _)| display_width(v)).max().unwrap_or(4).max(4).min(20);

        let header = format!(
            "{}{}{}",
            pad("软件", name_w + 2),
            pad("版本", ver_w + 1),
            pad("最佳速度", 12),
        );
        println!("{}  状态", header);
        println!("{}", "-".repeat(display_width(&header) + 20));

        let mut avail_count = 0;
        for (name, version, available, best) in &summary {
            let name_d = downloader::truncate_display(name, name_w);
            let ver_d = downloader::truncate_display(version, ver_w);
            let speed_str = match best {
                Some(s) => format!("\x1b[32m{:>10}\x1b[0m", format_size(s * 1024.0) + "/s"),
                None => pad("-", 10),
            };
            let status = if *available {
                avail_count += 1;
                "\x1b[32m可用\x1b[0m"
            } else {
                "\x1b[33m不可用\x1b[0m"
            };
            println!(
                "  {}{}{}  {}",
                pad(&name_d, name_w + 2),
                pad(&ver_d, ver_w + 1),
                speed_str,
                status,
            );
        }

        let unavailable = summary.len() - avail_count;
        print!("\n\x1b[90m总计: {} 个软件 | \x1b[32m{} 可用\x1b[0m | \x1b[33m{} 不可用\x1b[0m    耗时 {:.0}s\x1b[0m",
            summary.len(), avail_count, unavailable, elapsed);

        if unavailable > 0 {
            println!("\n\n  \x1b[33m⚠ 以下软件所有源均不可用:\x1b[0m");
            for (name, _, _, _) in &summary {
                if let Some((_, urls)) = by_sw.get(name) {
                    let all_dead = urls.iter().all(|r| r.3.is_none());
                    if all_dead {
                        for (_disp, _ver, url, _sp) in urls.iter() {
                            println!("    \x1b[90m{}:\x1b[0m {}", name, url);
                        }
                    }
                }
            }
        }
        println!();
    } else {
        // ── 以源为单位统计（原有行为） ──
        let mut avail: Vec<_> = results.iter()
            .filter(|(_, _, _, s)| s.is_some())
            .collect();
        avail.sort_by(|a, b| a.3.unwrap().partial_cmp(&b.3.unwrap()).unwrap_or(std::cmp::Ordering::Equal));

        let name_w = avail.iter()
            .map(|(d, _, _, _)| display_width(d))
            .max()
            .unwrap_or(4)
            .max(4)
            .min(20);

        let ver_w = avail.iter()
            .map(|(_, v, _, _)| display_width(v))
            .max()
            .unwrap_or(4)
            .max(4)
            .min(16);

        let header = format!(
            "{}{}{}  源",
            pad("软件", name_w + 2),
            pad("版本", ver_w + 1),
            pad("速度", 12),
        );
        println!("{}", header);
        println!("{}", "-".repeat(display_width(&header) + 30));

        for (display, version, url, speed) in &avail {
            let s = speed.unwrap();
            let marker = format!("\x1b[32m{:>10}\x1b[0m", format_size(s * 1024.0) + "/s");
            let name_d = downloader::truncate_display(display, name_w);
            let ver_d = downloader::truncate_display(version, ver_w);
            println!(
                "  {}{}{}  \x1b[90m{}\x1b[0m",
                pad(&name_d, name_w + 2),
                pad(&ver_d, ver_w + 1),
                marker,
                url,
            );
        }

        let unavailable = total - avail.len();
        print!("\n\x1b[90m总计: {} 个源 | \x1b[32m{} 可用\x1b[0m | \x1b[33m{} 不可用\x1b[0m    耗时 {:.0}s\x1b[0m",
            total, avail.len(), unavailable, elapsed);

        if unavailable > 0 {
            println!("\n\n  \x1b[33m⚠ 以下源不可用，建议检查或更新:\x1b[0m");
            for (display, _, url, speed) in results.iter() {
                if speed.is_none() {
                    println!("    \x1b[90m{}:\x1b[0m {}", display, url);
                }
            }
        }
        println!();
    }

    Ok(())
}
