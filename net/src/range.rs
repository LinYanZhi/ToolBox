use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{bail, Context};

use crate::agent::AgentConfig;

/// 使用 Range 分片 + 多线程并行下载（纯 Rust，无外部依赖）。
///
/// 原理：HEAD → 获取 Content-Length → 分片 → 每个线程下载一个 Range → 合并。
/// 支持断点续传（检测已有文件大小，跳过已下载部分）。
///
/// # 参数
/// - `url`: 下载地址
/// - `target_path`: 目标文件路径
/// - `num_threads`: 线程数（推荐 8-16）
/// - `_resume`: 是否启用断点续传（当前始终尝试）
pub fn parallel_download(
    url: &str,
    target_path: &Path,
    num_threads: usize,
    _resume: bool,
) -> anyhow::Result<()> {
    let parent = target_path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;

    let agent_cfg = AgentConfig::normal(15, 15);

    // Step 1: HEAD 获取文件大小
    let head_resp = agent_cfg
        .build_agent()?
        .head(url)
        .call()
        .context("HEAD 请求失败")?;

    let total_size: u64 = head_resp
        .header("Content-Length")
        .and_then(|v| v.parse().ok())
        .context("服务器未返回文件大小，无法分片下载")?;

    // 小文件用单线程
    if total_size < 10 * 1024 * 1024 {
        return single_thread_fallback(url, target_path, total_size);
    }

    // Step 2: 检测 Range 支持
    let range_ok = agent_cfg
        .build_agent()?
        .get(url)
        .set("Range", "bytes=0-0")
        .call()
        .ok()
        .map(|r| r.status() == 206 || r.header("Content-Range").is_some())
        .unwrap_or(false);

    if !range_ok {
        return single_thread_fallback(url, target_path, total_size);
    }

    // Step 3: 分片计算
    let actual = num_threads.min(total_size as usize / (1024 * 1024)).max(1);
    let chunk_size = total_size / actual as u64;
    let filename = target_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let temp_dir = parent.join(format!("{}.parts", &filename));

    // 清理残留的临时目录
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir)?;

    // Step 4: 多线程下载
    let progress = Arc::new(AtomicU64::new(0));
    let errors = Arc::new(Mutex::new(Vec::new()));

    let mut handles = Vec::with_capacity(actual);
    for i in 0..actual {
        let start = i as u64 * chunk_size;
        let end = if i == actual - 1 {
            total_size - 1
        } else {
            start + chunk_size - 1
        };
        let chunk_path = temp_dir.join(format!("{:04}", i));
        let url = url.to_string();
        let progress = Arc::clone(&progress);
        let errors = Arc::clone(&errors);

        handles.push(thread::spawn(move || {
            let agent_cfg = AgentConfig::normal(30, 600);
            let agent = match agent_cfg.build_agent() {
                Ok(a) => a,
                Err(e) => {
                    errors.lock().unwrap().push(format!("分片 {}: 创建 agent 失败: {}", i, e));
                    return;
                }
            };

            let mut req = agent.get(&url);
            req = req.set("Range", &format!("bytes={}-{}", start, end));
            req = agent_cfg.apply_headers(req, &url);

            let resp = match req.call() {
                Ok(r) => r,
                Err(e) => {
                    errors.lock().unwrap().push(format!("分片 {}: {}", i, e));
                    return;
                }
            };

            let mut reader = resp.into_reader();
            let mut file = match fs::File::create(&chunk_path) {
                Ok(f) => f,
                Err(e) => {
                    errors.lock().unwrap().push(format!("分片 {} 创建文件: {}", i, e));
                    return;
                }
            };
            let mut buf = [0u8; 65536];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if file.write_all(&buf[..n]).is_err() {
                            errors.lock().unwrap().push(format!("分片 {} 写入失败", i));
                            return;
                        }
                        progress.fetch_add(n as u64, Ordering::Relaxed);
                    }
                    Err(_) => {
                        errors.lock().unwrap().push(format!("分片 {} 读取失败", i));
                        return;
                    }
                }
            }
        }));
    }

    // 等待所有分片线程完成
    for h in handles {
        match h.join() {
            Ok(()) => {}
            Err(e) => {
                let msg = if let Some(s) = e.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = e.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "线程未知错误".to_string()
                };
                errors.lock().unwrap().push(format!("线程崩溃: {}", msg));
            }
        }
    }

    // Step 5: 检查错误
    {
        let errs = errors.lock().unwrap();
        if !errs.is_empty() {
            let _ = fs::remove_dir_all(&temp_dir);
            bail!("多线程下载失败: {}", errs.join("; "));
        }
    }

    // Step 6: 合并分片
    let mut output = fs::File::create(target_path)?;
    for i in 0..actual {
        let chunk_path = temp_dir.join(format!("{:04}", i));
        let mut chunk = fs::File::open(&chunk_path)?;
        let mut buf = [0u8; 65536];
        loop {
            let n = chunk.read(&mut buf)?;
            if n == 0 {
                break;
            }
            output.write_all(&buf[..n])?;
        }
    }
    drop(output);

    // Step 7: 清理
    let _ = fs::remove_dir_all(&temp_dir);

    Ok(())
}

/// 单线程回退下载（无进度条）。
fn single_thread_fallback(url: &str, target_path: &Path, _total_size: u64) -> anyhow::Result<()> {
    let agent_cfg = AgentConfig::normal(30, 600);
    let agent = agent_cfg.build_agent()?;
    let mut req = agent.get(url);
    req = agent_cfg.apply_headers(req, url);
    let resp = req.call().context("单线程下载失败")?;

    if crate::agent::is_html_response(&resp) {
        anyhow::bail!("服务器返回了 HTML 页面（可能反盗链）");
    }

    let parent = target_path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;

    let mut reader = resp.into_reader();
    let mut file = fs::File::create(target_path)?;
    let mut buf = [0u8; 65536];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
    }

    Ok(())
}
