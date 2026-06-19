use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result};
use ring::digest::{Context, SHA256};

/// 源仓库配置，管理内置镜像列表和用户自定义覆盖。
pub struct SourceRepo {
    /// 用户自定义源地址（环境变量 `AMINOS_SOURCE_REPO`）。
    pub custom: Option<String>,
    /// 内置镜像列表（按优先级排列）。
    pub builtin: Vec<String>,
}

impl SourceRepo {
    /// 创建源仓库配置，自动从环境变量读取自定义地址。
    pub fn new(builtin: Vec<String>) -> Self {
        let custom = std::env::var("AMINOS_SOURCE_REPO").ok();
        Self { custom, builtin }
    }

    /// 尝试顺序：自定义 > 内置镜像列表。
    pub fn resolve(&self) -> Vec<&str> {
        let mut repos: Vec<&str> = Vec::new();
        if let Some(ref c) = self.custom {
            repos.push(c);
        }
        for b in &self.builtin {
            if !repos.contains(&b.as_str()) {
                repos.push(b);
            }
        }
        repos
    }
}

/// 远程源定义索引（v2 格式）。
#[derive(serde::Deserialize)]
struct Index {
    #[allow(dead_code)]
    version: u32,
    /// 源最后更新时间（ISO 日期，如 "2026-06-15"）
    #[serde(default)]
    #[allow(dead_code)]
    updated: String,
    files: HashMap<String, FileEntry>,
}

#[derive(serde::Deserialize)]
struct FileEntry {
    sha256: String,
}

/// 从远程仓库增量更新源定义文件到本地目录。
///
/// 下载流程：
///   1. 依次尝试每个镜像，下载 `index.json`
///   2. 对比本地文件的 sha256，只下载新增/有变化的文件
///   3. 清理本地多余的文件
pub fn update_sources(source_dir: &Path, repo: &SourceRepo) -> Result<()> {
    fs::create_dir_all(source_dir)?;

    let repos = repo.resolve();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // 1. 下载 index.json
    let (raw_index_bytes, used_repo) = download_index(&repos, ts)?;

    // 去除 BOM（U+FEFF），PowerShell 的 UTF-8 输出可能带 BOM
    let index_bytes = if raw_index_bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &raw_index_bytes[3..]
    } else {
        &raw_index_bytes[..]
    };

    let index: Index = serde_json::from_slice(index_bytes)
        .context("源索引格式错误")?;

    // 2. 对比哈希，筛选出需要更新的文件
    let needs_update: Vec<String> = index
        .files
        .iter()
        .filter_map(|(fname, entry)| {
            let local_path = source_dir.join(fname);
            if sha256_file(&local_path).as_deref() == Some(&entry.sha256) {
                None // 哈希一致，跳过
            } else {
                Some(fname.clone()) // 新增或变更，需要下载
            }
        })
        .collect();

    // Pre-compute CJK-aware display width
    let max_name_w = index
        .files
        .keys()
        .map(|s| {
            use color::DisplayWidth;
            s.display_width()
        })
        .max()
        .unwrap_or(16)
        .max(16);

    println!(
        "  从 {} 下载，共 {} 个源文件（{} 个需更新，{} 个已是最新）",
        used_repo,
        index.files.len(),
        needs_update.len(),
        index.files.len() - needs_update.len()
    );

    // 3. 并发下载需要更新的文件
    let cache_bust = if used_repo.contains("jsdelivr") {
        format!("?v={}", ts)
    } else {
        String::new()
    };

    let (changed, unchanged_count, failed) = if needs_update.is_empty() {
        (0, 0, 0)
    } else {
        concurrent_download_files(&needs_update, source_dir, &repos, &cache_bust, max_name_w)?
    };

    let up_to_date = index.files.len() - needs_update.len() + unchanged_count;
    println!();
    let parts = Vec::from([
        (changed, "个更新"),
        (up_to_date, "个已最新"),
        (failed, "个失败"),
    ]);
    let summary: String = parts.iter()
        .filter_map(|(n, label)| if *n > 0 { Some(format!("{} {}", n, label)) } else { None })
        .collect::<Vec<_>>()
        .join("，");
    println!("  源更新完成。{}", summary);

    // 4. 清理本地多余文件
    let all_files: Vec<String> = index.files.into_keys().collect();
    cleanup_orphans(source_dir, &all_files, max_name_w);

    Ok(())
}

/// 尝试所有镜像，下载 index.json。
fn download_index(repos: &[&str], ts: u64) -> Result<(Vec<u8>, String)> {
    for repo in repos {
        let cache_bust = if repo.contains("jsdelivr") {
            format!("?v={}", ts)
        } else {
            String::new()
        };
        let index_url = format!("{}/index.json{}", repo, cache_bust);
        print!("  尝试: {} ... ", index_url);
        match download_bytes(&index_url) {
            Ok(data) => {
                println!("OK");
                return Ok((data, repo.to_string()));
            }
            Err(e) => {
                println!("失败 ({})", e);
            }
        }
    }
    anyhow::bail!("所有镜像均无法连接，请检查网络。首次使用请运行: as source update");
}

/// 并发下载文件（12 路线程池），失败时自动尝试下一个镜像。
///
/// 返回 (变更数, 未变数, 失败数)。
fn concurrent_download_files(
    files: &[String],
    source_dir: &Path,
    repos: &[&str],
    cache_bust: &str,
    max_name_w: usize,
) -> Result<(usize, usize, usize)> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;

    let files = Arc::new(files.to_owned());
    let next_idx = Arc::new(AtomicUsize::new(0));
    let changed = Arc::new(AtomicUsize::new(0));
    let unchanged = Arc::new(AtomicUsize::new(0));
    let failed = Arc::new(AtomicUsize::new(0));
    let source_dir = Arc::new(source_dir.to_path_buf());
    let repos: Arc<Vec<String>> = Arc::new(repos.iter().map(|s| s.to_string()).collect());
    let cache_bust = Arc::new(cache_bust.to_string());
    let print_lock = Arc::new(Mutex::new(()));

    let mut handles = Vec::new();
    for _ in 0..12 {
        let files = Arc::clone(&files);
        let next_idx = Arc::clone(&next_idx);
        let changed = Arc::clone(&changed);
        let unchanged = Arc::clone(&unchanged);
        let failed = Arc::clone(&failed);
        let source_dir = Arc::clone(&source_dir);
        let repos = Arc::clone(&repos);
        let cache_bust = Arc::clone(&cache_bust);
        let print_lock = Arc::clone(&print_lock);

        handles.push(thread::spawn(move || loop {
            let idx = next_idx.fetch_add(1, Ordering::Relaxed);
            if idx >= files.len() {
                break;
            }
            let fname = &files[idx];
            let dest = source_dir.join(fname);
            let padded_name = pad_left(fname, max_name_w);

            // 依次尝试所有镜像
            let mut last_err = String::new();
            let mut success = false;
            for repo in repos.iter() {
                let url = format!("{}/{}{}", repo, fname, cache_bust);
                match download_bytes(&url) {
                    Ok(data) => {
                        // 先读旧内容（必须在写入之前）
                        let old_content = fs::read(&dest).ok();

                        // 写入文件
                        if let Err(e) = fs::write(&dest, &data) {
                            let _g = print_lock.lock().unwrap();
                            eprintln!("  {}    写入失败: {}", padded_name, e);
                            last_err = e.to_string();
                            break;
                        }

                        // 判断状态
                        let is_new = old_content.is_none();
                        let is_changed = !is_new && old_content.as_deref() != Some(&data);
                        let status = if is_new {
                            changed.fetch_add(1, Ordering::Relaxed);
                            color::green("新增")
                        } else if is_changed {
                            changed.fetch_add(1, Ordering::Relaxed);
                            color::cyan("更新")
                        } else {
                            unchanged.fetch_add(1, Ordering::Relaxed);
                            color::gray("未变")
                        };

                        let _g = print_lock.lock().unwrap();
                        println!("  {}    {}  ({} B)", padded_name, status, data.len());

                        success = true;
                        break;
                    }
                    Err(e) => {
                        last_err = e.to_string();
                    }
                }
            }

            if !success {
                let _g = print_lock.lock().unwrap();
                println!("  {}    {}", padded_name, color::yellow(format!("失败 ({})", last_err)));
                failed.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    for h in handles {
        let _ = h.join();
    }

    Ok((
        changed.load(Ordering::Relaxed),
        unchanged.load(Ordering::Relaxed),
        failed.load(Ordering::Relaxed),
    ))
}

/// 删除 index 中不存在的本地文件。
fn cleanup_orphans(source_dir: &Path, files: &[String], max_name_w: usize) {
    let index_set: HashSet<&str> = files.iter().map(|s| s.as_str()).collect();
    if let Ok(entries) = fs::read_dir(source_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name == "index.json" {
                        continue;
                    }
                    if !index_set.contains(name) {
                        let _ = fs::remove_file(&path);
                        println!("  {}    {}", pad_left(name, max_name_w), color::red("删除"));
                    }
                }
            }
        }
    }
}

/// 计算文件的 SHA-256 哈希（十六进制小写）。
fn sha256_file(path: &Path) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut ctx = Context::new(&SHA256);
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        ctx.update(&buf[..n]);
    }
    Some(hex_encode(ctx.finish().as_ref()))
}

/// 将字节切片编码为小写十六进制字符串。
fn hex_encode(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len() * 2);
    for b in data {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// 从 URL 下载字节内容（轻量，无进度条）。
fn download_bytes(url: &str) -> Result<Vec<u8>> {
    let agent = ureq::AgentBuilder::new()
        .user_agent("aminos/0.1")
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(30))
        .build();

    let resp = agent
        .get(url)
        .call()
        .with_context(|| format!("下载失败: {}", url))?;

    if resp.status() >= 400 {
        anyhow::bail!("HTTP {}", resp.status());
    }

    let mut reader = resp.into_reader();
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    Ok(buf)
}

/// CJK 感知的左填充。
fn pad_left(s: &str, w: usize) -> String {
    use color::DisplayWidth;
    let dw = s.display_width();
    if dw >= w {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(w - dw))
    }
}
