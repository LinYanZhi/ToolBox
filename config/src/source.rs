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

    let updated = if needs_update.is_empty() {
        0
    } else {
        concurrent_download_files(&needs_update, source_dir, &used_repo, &cache_bust, max_name_w)?
    };

    println!("\n  源更新完成。{} 个更新，{} 个跳过", updated, index.files.len() - needs_update.len());

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

/// 并发下载文件（12 路线程池）。
fn concurrent_download_files(
    files: &[String],
    source_dir: &Path,
    repo: &str,
    cache_bust: &str,
    max_name_w: usize,
) -> Result<usize> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;

    let files = Arc::new(files.to_owned());
    let next_idx = Arc::new(AtomicUsize::new(0));
    let ok_count = Arc::new(AtomicUsize::new(0));
    let source_dir = Arc::new(source_dir.to_path_buf());
    let repo = Arc::new(repo.to_string());
    let cache_bust = Arc::new(cache_bust.to_string());
    let print_lock = Arc::new(Mutex::new(()));

    let mut handles = Vec::new();
    for _ in 0..12 {
        let files = Arc::clone(&files);
        let next_idx = Arc::clone(&next_idx);
        let ok_count = Arc::clone(&ok_count);
        let source_dir = Arc::clone(&source_dir);
        let repo = Arc::clone(&repo);
        let cache_bust = Arc::clone(&cache_bust);
        let print_lock = Arc::clone(&print_lock);

        handles.push(thread::spawn(move || loop {
            let idx = next_idx.fetch_add(1, Ordering::Relaxed);
            if idx >= files.len() {
                break;
            }
            let fname = &files[idx];
            let url = format!("{}/{}{}", repo, fname, cache_bust);
            let dest = source_dir.join(fname);

            let old_content = fs::read(&dest).ok();
            let padded_name = pad_left(fname, max_name_w);

            match download_bytes(&url) {
                Ok(data) => {
                    match fs::write(&dest, &data) {
                        Err(e) => {
                            let _g = print_lock.lock().unwrap();
                            eprintln!("  {}    写入失败 ({})", padded_name, e);
                        }
                        Ok(()) => {
                            let _g = print_lock.lock().unwrap();
                            let is_new = old_content.is_none();
                            let is_changed = !is_new && old_content.as_deref() != Some(&data);
                            let status = if is_new {
                                color::green("新增")
                            } else if is_changed {
                                color::cyan("更新")
                            } else {
                                color::gray("未变")
                            };
                            println!("  {}    {}  ({} B)", padded_name, status, data.len());
                            ok_count.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                Err(e) => {
                    let _g = print_lock.lock().unwrap();
                    println!("  {}    {}", padded_name, color::yellow(format!("跳过 ({})", e)));
                }
            }
        }));
    }

    for h in handles {
        let _ = h.join();
    }

    Ok(ok_count.load(Ordering::Relaxed))
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
