use std::fs;
use std::io::Read;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

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

/// 远程源定义索引。
#[derive(serde::Deserialize)]
struct Index {
    files: Vec<String>,
}

/// 从远程仓库更新源定义文件到本地目录。
///
/// 下载流程：
///   1. 依次尝试每个镜像，下载 `index.json`
///   2. 并发下载 index 中列出的所有文件（12 路并发）
///   3. 清理本地多余的文件
pub fn update_sources(source_dir: &std::path::Path, repo: &SourceRepo) -> Result<()> {
    fs::create_dir_all(source_dir)?;

    let repos = repo.resolve();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // 1. 下载 index.json
    let (index_bytes, used_repo) = download_index(&repos, ts)?;

    let index: Index = serde_json::from_slice(&index_bytes)
        .context("源索引格式错误")?;

    println!("  从 {} 下载，共 {} 个源文件", used_repo, index.files.len());

    // Pre-compute CJK-aware display width
    let max_name_w = index
        .files
        .iter()
        .map(|s| {
            use color::DisplayWidth;
            s.display_width()
        })
        .max()
        .unwrap_or(16)
        .max(16);

    // 2. 并发下载每个源定义文件
    let cache_bust = if used_repo.contains("jsdelivr") {
        format!("?v={}", ts)
    } else {
        String::new()
    };

    let ok_count = concurrent_download_files(
        &index.files,
        source_dir,
        &used_repo,
        &cache_bust,
        max_name_w,
    )?;

    println!("\n  源更新完成。{} 个成功", ok_count);

    // 3. 清理本地多余文件
    cleanup_orphans(source_dir, &index.files, max_name_w);

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
    anyhow::bail!("所有镜像均无法连接，请检查网络。也可将 source/ 文件夹放到 as.exe 同级目录");
}

/// 并发下载文件（12 路线程池）。
fn concurrent_download_files(
    files: &[String],
    source_dir: &std::path::Path,
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
fn cleanup_orphans(source_dir: &std::path::Path, files: &[String], max_name_w: usize) {
    let index_set: std::collections::HashSet<&str> = files.iter().map(|s| s.as_str()).collect();
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
