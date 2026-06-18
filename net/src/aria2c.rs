use std::io::BufRead;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context};

use crate::agent::Fingerprint;
use crate::download::{Cancel, ProgressCtx};

/// 使用系统 aria2c 多线程下载，实时跟踪进度。
///
/// 进度跟踪方式：
///   1. HEAD 获取 Content-Length
///   2. 管道 **stdout**（不是 stderr）
///   3. `read_line()` 按 `\n` 切分，解析 `[#GID cur/total(%) ...]` 格式
pub fn try_aria2c_download(
    url: &str,
    target_path: &Path,
    cancel: &Cancel,
    pb: Option<ProgressCtx>,
) -> anyhow::Result<()> {
    let aria2c = find_aria2c()
        .ok_or_else(|| anyhow::anyhow!("未找到 aria2c.exe"))?;

    let filename = target_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let parent = target_path.parent().unwrap_or(Path::new("."));

    // ── HEAD 获取 Content-Length（仅用于进度条初始长度） ──
    let total_size = probe_content_length(url);
    if let Some(ref ctx) = pb {
        if total_size > 0 {
            ctx.bar.set_length(total_size);
        }
    }

    // ── 第一步：分片模式（多线程） ──
    let split_args: &[&str] = &["-x", "16", "-s", "16", "-k", "1M"];
    let result = run_aria2c(&aria2c, url, target_path, &parent, &filename, split_args, cancel, pb.clone());
    if result.is_ok() {
        return result;
    }
    // 如果被取消，直接返回错误，不重试
    if cancel.is_cancelled() {
        return result;
    }

    // ── 第二步：分片失败 → 不分片模式（单线程重试） ──
    if let Some(ref ctx) = pb {
        eprintln!("  使用 {}（单线程重试）", ctx.name);
    }
    // 清理分片模式留下的残留文件
    let _ = std::fs::remove_file(target_path);
    // 也清理 aria2c 的 .downloading 控制文件
    let ctrl_name = format!("{}.downloading", target_path.to_string_lossy());
    let _ = std::fs::remove_file(&ctrl_name);

    let nosplit_args: &[&str] = &["-x", "1", "-s", "1"];
    run_aria2c(&aria2c, url, target_path, &parent, &filename, nosplit_args, cancel, pb)
}

/// 运行 aria2c 进程并等待完成。
fn run_aria2c(
    aria2c: &std::path::Path,
    url: &str,
    target_path: &Path,
    parent: &Path,
    filename: &str,
    extra_args: &[&str],
    cancel: &Cancel,
    pb: Option<ProgressCtx>,
) -> anyhow::Result<()> {
    let has_partial = Path::new(&format!("{}.downloading", target_path.to_string_lossy())).is_file();
    let target_exists = target_path.is_file();

    let mut cmd = Command::new(&aria2c);
    cmd.args(extra_args);
    cmd.args([
        "--retry-wait", "1",
        "--max-tries", "2",
        "--connect-timeout", "8",
        "--timeout", "120",
        "--allow-overwrite=true",
        "--auto-file-renaming=false",
        "--summary-interval", "1",
    ]);

    if has_partial || target_exists {
        cmd.arg("--continue=true");
    }

    cmd.args(["--dir", &parent.to_string_lossy(), "--out", &filename]);

    cmd.arg("--header");
    cmd.arg(format!("User-Agent: {}", Fingerprint::Chrome120.user_agent()));
    cmd.arg("--header");
    cmd.arg("Accept: */*");
    cmd.arg("--header");
    cmd.arg("Accept-Language: zh-CN,zh;q=0.9,en;q=0.8");

    let hostname = url.split("://").nth(1)
        .and_then(|s| s.split('/').next()).unwrap_or("");
    if !hostname.is_empty() {
        cmd.arg("--header");
        cmd.arg(format!("Referer: https://{}/", hostname));
    }

    cmd.arg(url);

    // ── 启动 aria2c（管道 stdout，stderr 静默） ──
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())   // aria2c 的进度行输出在 stdout，不是 stderr
        .stderr(Stdio::null())
        .spawn()
        .context("启动 aria2c 失败")?;

    let stdout = child.stdout.take().expect("aria2c stdout 已 piped");

    // ── Reader 线程：read_line() 按 \n 读取 stdout，解析进度 ──
    let reader_cancel = cancel.clone();
    let reader_pb = pb.clone();
    let reader_handle = thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stdout);
        let mut line = String::with_capacity(256);
        loop {
            if reader_cancel.is_cancelled() {
                return;
            }
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => return,
                Ok(_) => {}
                Err(_) => return,
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let (cur, total, _speed_str) = match parse_aria2_progress(trimmed) {
                Some((c, t, s)) => (c, t, s),
                None => continue,
            };
            reader_cancel.mark_progress();
            if let Some(ref ctx) = reader_pb {
                if total > 0 {
                    ctx.bar.set_length(total);
                }
                ctx.bar.set_position(cur);
                let prefix = if total > 0 {
                    crate::download::format_decimal_progress(cur, total)
                } else {
                    String::new()
                };
                // 不把速度塞进 prefix 了，speed 由模板的 {decimal_bytes_per_sec} 统一显示
                ctx.bar.set_prefix(prefix);
                if total > 0 {
                    crate::download::set_progress_eta(&ctx.bar);
                }
            }
        }
    });

    // ── 主线程：try_wait 轮询 ──
    loop {
        if cancel.is_cancelled() {
            let _ = child.kill();
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if cancel.is_cancelled() {
                    let _ = std::fs::remove_file(target_path);
                    return Err(anyhow::anyhow!("已取消"));
                }
                if !status.success() {
                    let _ = std::fs::remove_file(target_path);
                    bail!("aria2c 进程异常退出");
                }
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(e) => bail!("等待 aria2c 退出失败: {}", e),
        }
    }

    let _ = reader_handle.join();

    if !target_path.is_file()
        || std::fs::metadata(target_path).map(|m| m.len()).unwrap_or(0) == 0
    {
        let _ = std::fs::remove_file(target_path);
        bail!("aria2c 下载失败（文件不存在或为空）");
    }

    if let Some(ref ctx) = pb {
        if let Ok(meta) = std::fs::metadata(target_path) {
            ctx.bar.set_length(meta.len());
            ctx.bar.set_position(meta.len());
        }
    }

    Ok(())
}

fn parse_aria2_progress(line: &str) -> Option<(u64, u64, Option<String>)> {
    // 实际 stdout 输出格式: [#6ec300 205MiB/228MiB(89%) CN:16 DL:3.9MiB ETA:5s]
    // 无 "SIZE:" 前缀。合并的 \r 块可能包含多段，只取第一个内容段。
    let segment = line.split(|c: char| c == '\r' || c == '\n').next()?;
    if !segment.starts_with("[#") {
        return None;
    }
    let rest = segment.splitn(2, ' ').nth(1)?;
    let slash = rest.find('/')?;
    let cur_str = &rest[..slash];
    let after_total = rest[slash + 1..]
        .find(|c: char| c != '.' && !c.is_ascii_alphanumeric())?;
    let total_str = &rest[slash + 1..slash + 1 + after_total];

    let cur = parse_aria2_size(cur_str)?;
    let total = parse_aria2_size(total_str)?;

    // 提取 DL: 后面的速度（如 DL:3.9MiB → "3.9MiB/s"）
    let speed = segment.split(' ').find_map(|part| {
        let part = part.trim();
        part.strip_prefix("DL:").map(|v| {
            let v = v.trim();
            if v.ends_with("MiB") || v.ends_with("KiB") || v.ends_with("GiB") {
                format!("{}/s", v)
            } else if v.ends_with('B') {
                format!("{}/s", v)
            } else {
                format!("{}B/s", v)
            }
        })
    });

    Some((cur, total, speed))
}

fn parse_aria2_size(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (num_str, unit) = if s.ends_with("KiB") {
        (&s[..s.len() - 3], 1024u64)
    } else if s.ends_with("MiB") {
        (&s[..s.len() - 3], 1024u64 * 1024)
    } else if s.ends_with("GiB") {
        (&s[..s.len() - 3], 1024u64 * 1024 * 1024)
    } else if s.ends_with("TiB") {
        (&s[..s.len() - 3], 1024u64 * 1024 * 1024 * 1024)
    } else {
        let v: u64 = s.parse().ok()?;
        return Some(v);
    };
    let val: f64 = num_str.parse().ok()?;
    Some((val * unit as f64) as u64)
}

fn probe_content_length(url: &str) -> u64 {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(5))
        .user_agent(Fingerprint::Chrome120.user_agent())
        .build();
    match agent.head(url).call() {
        Ok(resp) => resp.header("Content-Length")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0),
        Err(_) => 0,
    }
}

fn find_aria2c() -> Option<std::path::PathBuf> {
    // 优先使用工具目录中的 aria2c
    if let Some(tools_dir) = crate::backend::get_tools_bin_dir() {
        let candidate = tools_dir.join("aria2c.exe");
        if candidate.is_file() { return Some(candidate); }
    }
    if let Ok(path) = std::env::var("AMINOS_ARIA2C_PATH") {
        let p = std::path::PathBuf::from(path);
        if p.is_file() { return Some(p); }
    }
    if let Some(localappdata) = std::env::var_os("LOCALAPPDATA") {
        let candidate = std::path::PathBuf::from(localappdata)
            .join("aminos").join("tools").join("aria2c").join("aria2c.exe");
        if candidate.is_file() { return Some(candidate); }
    }
    if let Some(desktop) = std::env::var_os("USERPROFILE")
        .map(|p| std::path::PathBuf::from(p).join("Desktop").join("aria2c.exe"))
    { if desktop.is_file() { return Some(desktop); } }
    std::env::var_os("PATH").and_then(|paths| {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join("aria2c.exe");
            if candidate.is_file() { return Some(candidate); }
        }
        None
    })
}
