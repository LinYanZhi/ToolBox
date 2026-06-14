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
/// - `cancel`: 取消令牌，用于优雅终止
/// - `pb`: 外部进度条（可选），如果提供则使用它而不是创建新进度条
pub fn parallel_download(
    url: &str,
    target_path: &Path,
    num_threads: usize,
    _resume: bool,
    cancel: &crate::download::Cancel,
    pb: Option<crate::download::ProgressCtx>,
) -> anyhow::Result<()> {
    let parent = target_path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;

    if cancel.is_cancelled() {
        return Err(anyhow::anyhow!("已取消"));
    }

    let agent_cfg = AgentConfig::normal(15, 15);

    // Step 1: HEAD 获取文件大小
    let head_resp = match agent_cfg.build_agent()?.head(url).call() {
        Ok(r) => r,
        Err(_e) => {
            // HEAD 失败（服务器不支持的 HEAD），降级为普通 GET 下载
            return no_range_download(url, target_path, cancel, pb);
        }
    };

    let total_size: u64 = head_resp
        .header("Content-Length")
        .and_then(|v| v.parse().ok())
        .context("服务器未返回文件大小，无法分片下载")?;

    // 小文件用单线程
    if total_size < 10 * 1024 * 1024 {
        return single_thread_fallback(url, target_path, total_size, cancel, pb);
    }

    if cancel.is_cancelled() {
        return Err(anyhow::anyhow!("已取消"));
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
        return single_thread_fallback(url, target_path, total_size, cancel, pb);
    }

    // Step 3: 分片计算
    let actual = num_threads.min(total_size as usize / (1024 * 1024)).max(1);
    let chunk_size = total_size / actual as u64;
    let filename = target_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    // 加入 PID 防止多终端并发冲突
    let pid = std::process::id();
    let temp_dir = parent.join(format!("{}.parts.{}", &filename, pid));

    // 清理残留的临时目录
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir)?;

    // Step 4: 多线程下载
    let progress = Arc::new(AtomicU64::new(0));
    let errors = Arc::new(Mutex::new(Vec::new()));

    // 全局进度条（通过共享 MultiProgress 管理，避免多进度条打架）
    let external_pb = pb.is_some();
    let pb = pb.unwrap_or_else(|| {
        let bar = crate::download::progress().add(indicatif::ProgressBar::new(total_size));
        bar.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{msg:.green} [{bar:30.green/white}] {bytes:.green}/{total_bytes:.green} ({bytes_per_sec:.green}, {eta})")
                .unwrap()
                .progress_chars("━━━"),
        );
        bar.set_message("分片下载");
        crate::download::ProgressCtx::new(bar, "RustRange", "多线程")
    });
    let bar = pb.bar.clone();
    // 无论外部还是内部 bar，都必须把 length 设为真实文件大小。
    // 外部 bar 初始化时 length=1（new(1)），不能跳过 set_length。
    bar.set_length(total_size);

    // 进度监控线程
    let first_byte_flag = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let first_byte_flag_c = Arc::clone(&first_byte_flag);
    let pb_progress = Arc::clone(&progress);
    let pb_cancel = cancel.clone();
    let pb_clone = pb.clone();
    let pb_handle = {
        let bar = bar.clone();
        thread::spawn(move || {
            loop {
                if pb_cancel.is_cancelled() {
                    return;
                }
                let cur = pb_progress.load(Ordering::Relaxed);
                // 首个字节后更新状态为"下载中"
                if external_pb && first_byte_flag_c.load(Ordering::Relaxed) && cur > 0 {
                    first_byte_flag_c.store(false, Ordering::Relaxed);
                    pb_clone.set_status("下载中");
                }
                bar.set_position(cur);
                if cur >= total_size {
                    break;
                }
                thread::sleep(std::time::Duration::from_millis(200));
            }
        })
    };

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
        let cancel = cancel.clone();

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
                        cancel.mark_progress();
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
        // 如果已取消，不再等待后续线程
        if cancel.is_cancelled() {
            let _ = fs::remove_dir_all(&temp_dir);
            return Err(anyhow::anyhow!("已取消"));
        }
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

    // 停止进度监控并刷新到 100%
    let _ = pb_handle.join();
    pb.bar.set_position(total_size);

    // Step 5: 检查错误
    {
        let errs = errors.lock().unwrap();
        if !errs.is_empty() {
            if !external_pb { pb.bar.abandon_with_message("下载失败"); }
            let _ = fs::remove_dir_all(&temp_dir);
            bail!("多线程下载失败: {}", errs.join("; "));
        }
    }

    // Step 6: 合并分片
    if !external_pb { pb.bar.set_message("合并分片"); }
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
    if !external_pb { pb.bar.finish_with_message("下载完成"); } else { pb.set_status("✓ 完成"); }

    Ok(())
}

/// 当 HEAD 或 Range 均不可用时，降级为普通 GET 单线程下载。
/// 不需要预先知道文件大小，不使用 Range 分片。
pub(crate) fn no_range_download(
    url: &str,
    target_path: &Path,
    cancel: &crate::download::Cancel,
    pb: Option<crate::download::ProgressCtx>,
) -> anyhow::Result<()> {
    if cancel.is_cancelled() {
        return Err(anyhow::anyhow!("已取消"));
    }

    let agent_cfg = AgentConfig::normal(30, 600);
    let agent = agent_cfg.build_agent()?;
    let mut req = agent.get(url);
    req = agent_cfg.apply_headers(req, url);
    let resp = req.call().context("GET 下载失败")?;

    if crate::agent::is_html_response(&resp) {
        anyhow::bail!("服务器返回了 HTML 页面（可能反盗链）");
    }

    let parent = target_path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;

    // 尝试从响应头获取文件大小
    let total_size: u64 = resp.header("Content-Length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    if let Some(ref ctx) = pb {
        if total_size > 0 {
            ctx.bar.set_length(total_size);
        }
        ctx.set_status("下载中");
    }

    let mut reader = resp.into_reader();
    let mut file = if cfg!(windows) {
        // Windows 上用 create + truncate
        let f = fs::File::create(target_path)?;
        f.set_len(0).ok();
        f
    } else {
        fs::File::create(target_path)?
    };
    let mut buf = [0u8; 65536];

    loop {
        if cancel.is_cancelled() {
            if let Some(ref ctx) = pb {
                ctx.set_status("✗ 已取消");
            }
            return Err(anyhow::anyhow!("已取消"));
        }
        let n = reader.read(&mut buf)?;
        if n == 0 { break; }
        file.write_all(&buf[..n])?;
        if let Some(ref ctx) = pb {
            ctx.bar.inc(n as u64);
            if total_size > 0 {
                ctx.bar.set_length(total_size);
            }
        }
    }

    if let Some(ref ctx) = pb {
        ctx.set_status("✓ 完成");
    }
    Ok(())
}

/// 单线程回退下载（含进度条）。
fn single_thread_fallback(url: &str, target_path: &Path, total_size: u64, cancel: &crate::download::Cancel, pb: Option<crate::download::ProgressCtx>) -> anyhow::Result<()> {
    if cancel.is_cancelled() {
        return Err(anyhow::anyhow!("已取消"));
    }

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

    let external_pb = pb.is_some();
    let pb = if let Some(ctx) = pb {
        // 外部传入的进度条上下文 — 使用它但不覆盖 status（会在下载开始时更新）
        if total_size > 0 {
            ctx.bar.set_length(total_size);
        }
        Some(ctx)
    } else if total_size > 0 {
        let bar = crate::download::progress().add(indicatif::ProgressBar::new(total_size));
        bar.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{msg:.green} [{bar:30.green/white}] {bytes:.green}/{total_bytes:.green} ({bytes_per_sec:.green}, {eta})")
                .unwrap()
                .progress_chars("━━━"),
        );
        bar.set_message("单线程下载");
        Some(crate::download::ProgressCtx::new(bar, "RustRange", "单线程"))
    } else {
        None
    };
    let mut first_byte = true;

    let mut reader = resp.into_reader();
    let mut file = fs::File::create(target_path)?;
    let mut buf = [0u8; 65536];
    loop {
        if cancel.is_cancelled() {
            if let Some(ref ctx) = pb {
                ctx.set_status("✗ 已取消");
            }
            let _ = fs::remove_file(target_path);
            return Err(anyhow::anyhow!("已取消"));
        }
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        if first_byte {
            first_byte = false;
            if let Some(ref ctx) = pb {
                if external_pb { ctx.set_status("下载中"); }
            }
        }
        file.write_all(&buf[..n])?;
        cancel.mark_progress();
        if let Some(ref ctx) = pb {
            ctx.bar.inc(n as u64);
        }
    }

    if let Some(ctx) = pb {
        if external_pb { ctx.set_status("✓ 完成"); } else { ctx.bar.finish_with_message("下载完成"); }
    }

    Ok(())
}
