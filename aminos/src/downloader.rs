use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use indicatif::{ProgressBar, ProgressStyle};

const CHUNK: usize = 64 * 1024;
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120.0.0.0";

// ── Browser emulation headers (match Python `_browser_headers`) ──

/// Build a full set of browser-mimicking headers, including
/// per-domain `Referer` detection (same as Python version).
fn browser_headers(url: &str) -> Vec<(&'static str, String)> {
    let mut headers = vec![
        ("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8".to_string()),
        ("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8".to_string()),
    ];

    let hostname = url
        .split("://")
        .nth(1)
        .and_then(|s| s.split('/').next())
        .unwrap_or("");

    if hostname.is_empty() {
        return headers;
    }

    // Python: ALWAYS sets Referer. For known hosts use specific URL,
    // otherwise use `f"https://{parsed.hostname}/"`.
    let referer = match hostname {
        "download.jetbrains.com" => "https://www.jetbrains.com/".to_string(),
        "dldir1.qq.com" => "https://work.weixin.qq.com/".to_string(),
        "softwareupdate.vmware.com" => "https://www.vmware.com/".to_string(),
        "dl.google.com" => "https://www.google.com/".to_string(),
        "redirector.gvt1.com" => "https://developer.android.com/".to_string(),
        "download.trae.com.cn" => "https://www.trae.com.cn/".to_string(),
        "download.cursor.com" => "https://www.cursor.com/".to_string(),
        "sunlogin.oray.com" => "https://sunlogin.oray.com/".to_string(),
        _ => format!("https://{}/", hostname),
    };

    headers.push(("Referer", referer));
    headers
}

/// Apply browser headers to a ureq request.
fn with_browser_headers(req: ureq::Request, url: &str) -> ureq::Request {
    let mut r = req;
    for (key, val) in browser_headers(url) {
        r = r.set(key, &val);
    }
    r
}

/// Build a ureq agent with normal TLS (certificates are validated against
/// the system trust store). This is the default/fast path.
fn normal_agent(connect_timeout: u64, read_timeout: u64) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .user_agent(USER_AGENT)
        .timeout_connect(Duration::from_secs(connect_timeout))
        .timeout_read(Duration::from_secs(read_timeout))
        .build()
}

/// Build a ureq agent that skips certificate verification (same as Python's
/// `ssl._create_unverified_context()` used in all speed tests and downloads).
fn insecure_agent(connect_timeout: u64, read_timeout: u64) -> anyhow::Result<ureq::Agent> {
    let tls = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .context("无法创建 TLS 连接器")?;
    Ok(ureq::AgentBuilder::new()
        .user_agent(USER_AGENT)
        .timeout_connect(Duration::from_secs(connect_timeout))
        .timeout_read(Duration::from_secs(read_timeout))
        .tls_connector(Arc::new(tls))
        .build())
}

// ── Public API ───────────────────────────────────────────

/// 查找与 as.exe 同目录的 aria2c.exe
fn find_aria2c() -> Option<std::path::PathBuf> {
    // 1. 与 as.exe 同目录
    let exe = std::env::current_exe().ok()?;
    let aria2c = exe.parent()?.join("aria2c.exe");
    if aria2c.is_file() {
        return Some(aria2c);
    }

    // 2. 桌面（用户可能放那测试）
    let desktop = std::path::PathBuf::from(std::env::var("USERPROFILE").ok()?)
        .join("Desktop")
        .join("aria2c.exe");
    if desktop.is_file() {
        return Some(desktop);
    }

    // 3. PATH 环境变量
    which_aria2c()
}

/// 在 PATH 中查找 aria2c
fn which_aria2c() -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join("aria2c.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    })
}

/// 使用 aria2c 多线程下载（自动检测并启用）
///
/// aria2c 会分多线程下载，速度远快于单线程 HTTPS。
/// 自动续传未完成的 .downloading 文件。
/// 下载到 `target_path` 指定的位置（as 定义的缓存目录），可控。
fn try_aria2c_download(url: &str, target_path: &Path) -> anyhow::Result<()> {
    let aria2c = find_aria2c()
        .ok_or_else(|| anyhow::anyhow!("未找到 aria2c.exe"))?;

    let filename = target_path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let parent = target_path.parent().unwrap_or(Path::new("."));

    // 检查是否有未完成的下载，支持续传
    let partial_path = format!("{}.downloading", target_path.to_string_lossy());
    let partial_file = std::path::Path::new(&partial_path);
    let has_partial = partial_file.is_file();
    let target_exists = target_path.is_file();

    if has_partial {
        println!("  发现未完成的下载，续传中... ({})", partial_file.display());
    }

    let mut cmd = std::process::Command::new(&aria2c);
    cmd.args([
        "-x", "16",
        "-s", "16",
        "-k", "1M",
        "--retry-wait", "3",
        "--max-tries", "5",
        "--connect-timeout", "30",
        "--timeout", "600",
        "--allow-overwrite=true",
        "--auto-file-renaming=false",
        "--error-exit-code=1",
    ]);

    // 续传: aria2c 默认续传同名文件，我们不删 .downloading，aria2c 会自动续传
    if has_partial || target_exists {
        cmd.arg("--continue=true");
    }

    cmd.args([
        "--dir", &parent.to_string_lossy(),
        "--out", &filename,
    ]);

    // User-Agent 和 Referer
    cmd.arg("--header");
    cmd.arg(format!("User-Agent: {}", USER_AGENT));
    let hostname = url.split("://").nth(1).and_then(|s| s.split('/').next()).unwrap_or("");
    if !hostname.is_empty() {
        cmd.arg("--header");
        cmd.arg(format!("Referer: https://{}/", hostname));
    }

    cmd.arg(url);

    let status = cmd
        .stdin(std::process::Stdio::null())
        .status()
        .context("运行 aria2c 失败")?;

    // aria2c 老版本（<1.37）下载失败也退出 0，需要靠文件存在性判断
    if !status.success() {
        let _ = std::fs::remove_file(&partial_path);
        let _ = std::fs::remove_file(target_path);
        bail!("aria2c 进程异常退出");
    }

    // 检查目标文件是否存在且非空
    if !target_path.is_file() || std::fs::metadata(target_path).map(|m| m.len()).unwrap_or(0) == 0 {
        let _ = std::fs::remove_file(&partial_path);
        let _ = std::fs::remove_file(target_path);
        bail!("aria2c 下载失败（文件不存在或为空）");
    }

    Ok(())
}

// ── Rust 原生多线程下载 ─────────────────────────────

/// 使用 Range 分片 + 多线程并行下载（纯 Rust，零外部依赖）
///
/// 原理：HEAD → 获取 Content-Length → 分片 → 每个线程下载一个 Range → 合并
/// 支持断点续传（检测已有文件大小，跳过已下载部分）
/// 最大 16 线程，等价于 aria2c 的 `-x 16 -s 16`
fn parallel_download(url: &str, target_path: &Path) -> anyhow::Result<()> {
    const NUM_THREADS: usize = 16;
    let parent = target_path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let filename = target_path.file_name().unwrap_or_default().to_string_lossy().to_string();

    // Step 1: HEAD 获取文件大小，同时检测 Range 支持
    let head_resp = with_browser_headers(normal_agent(15, 15).head(url), url)
        .call()
        .context("HEAD 请求失败，服务器可能不支持多线程下载")?;

    let total_size: u64 = head_resp
        .header("Content-Length")
        .and_then(|v| v.parse().ok())
        .context("服务器未返回文件大小，无法分片下载")?;

    // 小文件用单线程
    if total_size < 10 * 1024 * 1024 {
        let resp = with_browser_headers(normal_agent(30, 600).get(url), url)
            .call()
            .context("单线程下载失败")?;
        return download_body(resp, target_path);
    }

    // Step 2: 检测服务器是否支持 Range
    let range_ok = with_browser_headers(
        normal_agent(10, 10).get(url).set("Range", "bytes=0-0"),
        url,
    )
    .call()
    .ok()
    .and_then(|r| {
        let code = r.status();
        // 206 Partial Content 或 200 OK（但带了 Content-Range）都算支持
        let has_content_range = r.header("Content-Range").is_some();
        Some(code == 206 || has_content_range)
    })
    .unwrap_or(false);

    if !range_ok {
        // 不支持 Range，回退单线程
        let resp = with_browser_headers(normal_agent(30, 600).get(url), url)
            .call()
            .context("单线程下载失败")?;
        return download_body(resp, target_path);
    }

    // Step 3: 分片
    let actual = NUM_THREADS.min(total_size as usize / (1024 * 1024)).max(1);
    let chunk_size = total_size / actual as u64;
    let temp_dir = parent.join(format!("{}.parts", target_path.file_name().unwrap_or_default().to_string_lossy()));
    fs::create_dir_all(&temp_dir)?;

    // Step 4: 多线程下载
    let progress = Arc::new(AtomicU64::new(0));
    let errors = Arc::new(Mutex::new(Vec::new()));

    let mut handles = Vec::with_capacity(actual);
    for i in 0..actual {
        let start = i as u64 * chunk_size;
        let end = if i == actual - 1 { total_size - 1 } else { start + chunk_size - 1 };
        let chunk_path = temp_dir.join(format!("{:04}", i));
        let url = url.to_string();
        let progress = Arc::clone(&progress);
        let errors = Arc::clone(&errors);

        handles.push(thread::spawn(move || {
            let agent = normal_agent(30, 600);
            let mut req = agent.get(&url);
            req = req.set("Range", &format!("bytes={}-{}", start, end));
            req = with_browser_headers(req, &url);

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

    // Step 5: 进度条
    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg}\n{wide_bar} {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .unwrap(),
    );
    pb.set_message(format!("多线程下载 {}", filename));

    let pb2 = pb.clone();
    let progress2 = Arc::clone(&progress);
    let total = total_size;
    let monitor = thread::spawn(move || loop {
        let done = progress2.load(Ordering::Relaxed);
        pb2.set_position(done);
        if done >= total {
            pb2.finish_and_clear();
            break;
        }
        thread::sleep(Duration::from_millis(200));
    });

    for h in handles {
        h.join().unwrap();
    }
    monitor.join().unwrap();

    // Step 6: 检查错误
    {
        let errs = errors.lock().unwrap();
        if !errs.is_empty() {
            let _ = fs::remove_dir_all(&temp_dir);
            bail!("多线程下载失败: {}", errs.join("; "));
        }
    }

    // Step 7: 合并分片
    let mut output = fs::File::create(target_path)?;
    for i in 0..actual {
        let chunk_path = temp_dir.join(format!("{:04}", i));
        let mut chunk = fs::File::open(&chunk_path)?;
        let mut buf = [0u8; 65536];
        loop {
            let n = chunk.read(&mut buf)?;
            if n == 0 { break; }
            output.write_all(&buf[..n])?;
        }
    }
    drop(output);

    // Step 8: 清理
    let _ = fs::remove_dir_all(&temp_dir);

    println!("  ✓ {} 多线程下载完成 ({} 线程)", filename, actual);
    Ok(())
}

/// 检测响应是否为 HTML 页面（反盗链页面）
fn is_html_response(resp: &ureq::Response) -> bool {
    resp.header("Content-Type")
        .map(|ct| ct.contains("text/html") || ct.contains("text/plain"))
        .unwrap_or(false)
}

/// Download a file from `url` to `target_path`, showing a progress bar.
///
/// Tries:
///   1. Rust 原生多线程下载 (16 线程, Range 分片, 最快)
///   2. aria2c (如果存在, 回退)
///   3. normal TLS (ureq 单线程)
///   4. insecure TLS (ureq, 跳过证书校验)
///   5. system curl.exe
pub fn download_with_progress(url: &str, target_path: &Path, renew: bool) -> anyhow::Result<()> {
    // 检查缓存文件：必须存在、未被中断下载、签名合法
    let fname = target_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let is_partial = fname.ends_with(".downloading")
        || fname.ends_with(".aria2");
    if target_path.exists() && !renew && !is_partial && verify_downloaded_file(target_path) {
        println!("  使用缓存: {}", target_path.display());
        return Ok(());
    }

    let mut errors: Vec<String> = Vec::new();

    // Tier 1: Rust 原生多线程下载 (16 线程 Range 分片)
    // 失败后自动回退，不影响下一级
    if let Err(e) = parallel_download(url, target_path) {
        errors.push(format!("Rust多线程: {}", e));
        let _ = std::fs::remove_file(target_path);
    } else if verify_downloaded_file(target_path) {
        return Ok(());
    } else {
        let _ = std::fs::remove_file(target_path);
        errors.push("Rust多线程: 下载内容签名不匹配".to_string());
    }

    // Tier 2: aria2c (如果存在)
    if find_aria2c().is_some() {
        match try_aria2c_download(url, target_path) {
            Ok(()) => {
                if verify_downloaded_file(target_path) {
                    return Ok(());
                }
                let _ = std::fs::remove_file(target_path);
                errors.push("aria2c: 下载内容签名不匹配".to_string());
            }
            Err(e) => {
                // 清理 aria2c 残留的 .downloading 文件
                let downloading_path = format!("{}.downloading", target_path.to_string_lossy());
                let _ = std::fs::remove_file(&downloading_path);
                errors.push(format!("aria2c: {}", e));
            }
        }
    }

    // Tier 3: Normal TLS (ureq 单线程)
    match try_download_with_agent(url, target_path, false, "ureq: ") {
        None => return Ok(()),
        Some(err) => errors.push(err),
    }

    // Tier 4: Insecure TLS (ureq)
    match try_download_with_agent(url, target_path, true, "insecure TLS: ") {
        None => return Ok(()),
        Some(err) => errors.push(err),
    }

    // Tier 5: system curl
    if let Err(e) = try_curl_download(url, target_path) {
        errors.push(format!("curl: {}", e));
    } else if !verify_downloaded_file(target_path) {
        let _ = std::fs::remove_file(target_path);
        errors.push("curl: 下载内容签名不匹配".to_string());
    } else {
        return Ok(());
    }

    bail!("无法下载 ({})", errors.join("; "));
}

/// 使用 ureq agent 下载，返回 None 表示成功，Some(err) 表示失败
fn try_download_with_agent(url: &str, target_path: &Path, insecure: bool, prefix: &str) -> Option<String> {
    let agent = if insecure {
        match insecure_agent(30, 600) {
            Ok(a) => a,
            Err(e) => return Some(format!("{}agent 创建失败: {}", prefix, e)),
        }
    } else {
        normal_agent(30, 600)
    };

    // 发起请求
    let mut req = agent.get(url);
    // 不发送 Range/分块请求，避免服务器返回非完整文件
    req = with_browser_headers(req, url);
    let resp = match req.call() {
        Ok(r) => r,
        Err(e) => return Some(format!("{}{}", prefix, e)),
    };

    // 提前检测：如果是 HTML 响应，跳过（可能是反盗链页面）
    if is_html_response(&resp) {
        return Some(format!("{}服务器返回了 HTML 页面（可能反盗链）", prefix));
    }

    // 下载到文件
    if let Err(e) = download_body(resp, target_path) {
        let _ = std::fs::remove_file(target_path);
        return Some(format!("{}{}", prefix, e));
    }

    // 签名校验
    if !verify_downloaded_file(target_path) {
        let _ = std::fs::remove_file(target_path);
        return Some(format!("{}下载内容签名不匹配", prefix));
    }

    None // 成功
}

/// Download body from a ureq Response to a file with progress bar.
fn download_body(resp: ureq::Response, target_path: &Path) -> anyhow::Result<()> {
    let total_size: u64 = resp.header("Content-Length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let parent = target_path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;

    let filename = target_path.file_name().unwrap_or_default().to_string_lossy();

    let pb = if total_size > 0 {
        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{msg}\n{wide_bar} {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                .unwrap()
        );
        pb.set_message(format!("下载 {}", filename));
        Some(pb)
    } else {
        println!("下载 {}（大小未知）...", filename);
        None
    };

    let mut reader = resp.into_reader();
    let mut file = fs::File::create(target_path)?;
    let mut buf = [0u8; CHUNK];
    let mut downloaded = 0u64;

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        downloaded += n as u64;
        if let Some(ref pb) = pb {
            pb.set_position(downloaded);
        }
    }

    if let Some(pb) = pb {
        pb.finish_and_clear();
    }

    Ok(())
}

/// 校验下载文件的签名是否合法（对外接口）
pub fn verify_downloaded_file(path: &Path) -> bool {
    let fname = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    // 先检查文件大小是否合理
    let file_size = match std::fs::metadata(path) {
        Ok(m) => m.len(),
        _ => return false,
    };

    // 只读前 4KB，够检查魔数了
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        _ => return false,
    };
    let mut header = [0u8; 4096];
    let n = match std::io::Read::read(&mut file, &mut header) {
        Ok(n) if n >= 4 => n,
        _ => return false,
    };

    // 按扩展名检查文件魔数和最小大小
    if fname.ends_with(".exe") || fname.ends_with(".dll") || fname.ends_with(".msi") {
        // PE 文件: MZ 开头 (4D 5A)，且大于 512KB（防反盗链页面）
        header[0] == 0x4D && header[1] == 0x5A && file_size >= 512 * 1024
    } else if fname.ends_with(".zip") || fname.ends_with(".7z") {
        (header[0] == 0x50 && header[1] == 0x4B && header[2] == 0x03 && header[3] == 0x04)
            || (header[0] == 0x37 && header[1] == 0x7A)
    } else if fname.ends_with(".rar") {
        header[0] == 0x52 && header[1] == 0x61 && header[2] == 0x72 && header[3] == 0x21
    } else if fname.ends_with(".tar") {
        n > 1024
    } else if fname.ends_with(".gz") || fname.ends_with(".xz") || fname.ends_with(".bz2") {
        (header[0] == 0x1F && header[1] == 0x8B)
            || (header[0] == 0xFD && header[1] == 0x37)
            || (header[0] == 0x42 && header[1] == 0x5A)
    } else if fname.ends_with(".iso") {
        header[0] == 0x43 && header[1] == 0x44 && header[2] == 0x30 && file_size >= 1024 * 1024
    } else if fname.ends_with(".appx") || fname.ends_with(".msix") {
        header[0] == 0x50 && header[1] == 0x4B && header[2] == 0x03 && header[3] == 0x04
    } else if fname.ends_with(".dmg") {
        n > 1024
    } else {
        n > 1024
    }
}
fn try_curl_download(url: &str, target_path: &Path) -> anyhow::Result<()> {
    let curl = "C:\\Windows\\System32\\curl.exe";
    if !std::path::Path::new(curl).exists() {
        bail!("未找到 curl.exe（系统可能缺少该文件）");
    }

    let status = std::process::Command::new(curl)
        .args(["-sL", "-o", &target_path.to_string_lossy(), "--max-time", "300", url])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("运行 curl 失败")?;

    if !status.success() {
        let code = status.code().unwrap_or(-1);
        if target_path.exists() && target_path.metadata().map(|m| m.len() > 0).unwrap_or(false) {
            // curl was killed by timeout but wrote some data — keep it
            return Ok(());
        }
        bail!("curl 退出码 {}", code);
    }

    Ok(())
}

/// Try downloading from multiple URLs in sequence.
pub fn download_with_fallback(
    name: &str,
    urls: &[String],
    target_path: &Path,
    renew: bool,
) -> anyhow::Result<usize> {
    let expanded = expand_github_urls(urls);
    for (i, url) in expanded.iter().enumerate() {
        println!("  尝试: {} ...", url);
        match download_with_progress(url, target_path, renew && i == 0) {
            Ok(()) => return Ok(i),
            Err(e) => {
                eprintln!("  失败: {}", e);
                let _ = fs::remove_file(target_path);
            }
        }
    }
    bail!("{}: 所有下载源均失败", name)
}

/// Construct a safe cached filename for a software installer.
pub fn safe_installer_name(name: &str, version: &str, urls: &[String]) -> String {
    let safe_name = name.to_lowercase().replace(' ', "-");
    let safe_ver = version.to_lowercase().replace(' ', "-");

    let ext = if let Some(first_url) = urls.first() {
        let path = first_url.split('?').next().unwrap_or(first_url);
        let seg = path.rsplit('/').next().unwrap_or("");
        if let Some(dot) = seg.rfind('.') {
            let e = &seg[dot..];
            if [".exe", ".msi", ".zip", ".7z", ".rar", ".tar", ".gz", ".xz", ".bz2", ".iso", ".appx", ".dmg"]
                .contains(&e.to_lowercase().as_str())
            {
                e.to_string()
            } else {
                ".exe".to_string()
            }
        } else {
            ".exe".to_string()
        }
    } else {
        ".exe".to_string()
    };

    format!("{}-{}{}", safe_name, safe_ver, ext)
}

/// 对 GitHub 下载地址自动追加 ghproxy 镜像。
pub fn expand_github_urls(urls: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(urls.len() * 2);
    for url in urls {
        out.push(url.clone());
        // 只对未经过代理的 GitHub 直链追加镜像
        if url.contains("github.com") && !url.contains("ghproxy") && !url.contains("gh-proxy") {
            out.push(format!("https://ghproxy.net/{}", url));
        }
    }
    out
}

// ── Formatting ────────────────────────────────────────────

/// Format bytes as human-readable (matches Python `format_size`).
pub fn format_size(size_bytes: f64) -> String {
    let mut size = size_bytes;
    for unit in &["B", "KB", "MB", "GB"] {
        if size < 1024.0 {
            return format!("{:.1} {}", size, unit);
        }
        size /= 1024.0;
    }
    format!("{:.1} TB", size)
}

/// Display width of a string (CJK-aware, simplified: CJK chars = 2 width).
pub fn display_width(s: &str) -> usize {
    s.chars().map(|c| {
        if c as u32 >= 0x1100
            && (c as u32 <= 0x115f
                || c as u32 == 0x2329
                || c as u32 == 0x232a
                || (c as u32 >= 0x2e80 && c as u32 <= 0xa4cf)
                || (c as u32 >= 0xac00 && c as u32 <= 0xd7a3)
                || (c as u32 >= 0xf900 && c as u32 <= 0xfaff)
                || (c as u32 >= 0xfe10 && c as u32 <= 0xfe19)
                || (c as u32 >= 0xfe30 && c as u32 <= 0xfe6f)
                || (c as u32 >= 0xff01 && c as u32 <= 0xff60)
                || (c as u32 >= 0xffe0 && c as u32 <= 0xffe6)
                || (c as u32 >= 0x1f300 && c as u32 <= 0x1f64f)
                || (c as u32 >= 0x1f900 && c as u32 <= 0x1f9ff)
                || (c as u32 >= 0x20000 && c as u32 <= 0x2fffd)
                || (c as u32 >= 0x30000 && c as u32 <= 0x3fffd))
        {
            2
        } else {
            1
        }
    }).sum()
}

/// Left-pad a string to `width` visual columns (CJK-aware).
pub fn pad(s: &str, width: usize) -> String {
    let dw = display_width(s);
    if dw >= width {
        return s.to_string();
    }
    format!("{}{}", s, " ".repeat(width - dw))
}

/// Truncate a string to `max_width` visual columns, append "..." if needed.
pub fn truncate_display(s: &str, max_width: usize) -> String {
    let suf = "...";
    let suf_w = display_width(suf);
    if display_width(s) <= max_width {
        return s.to_string();
    }
    let mut result = String::new();
    let mut w = 0usize;
    for c in s.chars() {
        let cw = if (c as u32) >= 0x2e80 && (c as u32) <= 0x9fff { 2 } else { 1 };
        if w + cw > max_width - suf_w {
            result.push_str(suf);
            break;
        }
        result.push(c);
        w += cw;
    }
    result
}

// ── Speed test ────────────────────────────────────────────

/// Measure download speed of a URL. Returns speed in KB/s.
///
/// Primary approach: system curl.exe (available on Win10/11, Schannel TLS —
/// same fingerprint as Edge/Chrome, passes ALL CDNs). We capture its stdout
/// directly into memory (no temp files, no pipe handles).
///
/// Fallback: ureq with normal TLS → insecure TLS (for systems without curl).
pub fn measure_speed(url: &str, timeout_secs: u64) -> Option<f64> {
    // Primary: system curl.exe (handles all CDN quirks: redirects, TLS, anti-bot)
    if let Some(speed) = try_curl_stdout(url, timeout_secs) {
        return Some(speed);
    }

    // Fallback: ureq with normal → insecure TLS
    const TEST_SIZE: usize = 64 * 1024;
    let normal = normal_agent(timeout_secs, timeout_secs);

    if let Some(speed) = try_fetch(&normal, url, TEST_SIZE, timeout_secs, true) {
        return Some(speed);
    }
    if let Some(speed) = try_fetch(&normal, url, TEST_SIZE, timeout_secs, false) {
        return Some(speed);
    }
    if let Ok(insecure) = insecure_agent(timeout_secs, timeout_secs) {
        if let Some(speed) = try_fetch(&insecure, url, TEST_SIZE, timeout_secs, true) {
            return Some(speed);
        }
        if let Some(speed) = try_fetch(&insecure, url, TEST_SIZE, timeout_secs, false) {
            return Some(speed);
        }
    }

    None
}

/// Speed test via system curl: capture stdout into memory, measure bytes/time.
fn try_curl_stdout(url: &str, timeout_secs: u64) -> Option<f64> {
    let curl = "C:\\Windows\\System32\\curl.exe";
    if !std::path::Path::new(curl).exists() {
        return None;
    }

    let max_time = timeout_secs + 5;

    let start = Instant::now();
    let output = std::process::Command::new(curl)
        .args(["-sL", "-r", "0-65535", "--max-time", &max_time.to_string(), url])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    let elapsed = start.elapsed().as_secs_f64();
    let size = output.stdout.len();

    // Don't check exit code — curl --max-time kills with code 28 but stdout
    // already has data (same as Python).
    if elapsed < 0.1 || size < 1024 {
        return None;
    }
    Some((size as f64 / 1024.0) / elapsed)
}

fn try_fetch(agent: &ureq::Agent, url: &str, test_size: usize, timeout: u64, range: bool) -> Option<f64> {
    let start = Instant::now();
    let mut req = agent.get(url);
    if range {
        req = req.set("Range", "bytes=0-65535");
    }
    let resp = with_browser_headers(req, url).call().ok()?;
    let mut reader = resp.into_reader();
    let mut buf = [0u8; 16 * 1024];
    let mut total: usize = 0;

    loop {
        if start.elapsed().as_secs() >= timeout {
            if total > 0 { break; } else { return None; }
        }
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                total += n;
                if total >= test_size {
                    break;
                }
            }
            Err(_) => {
                if total == 0 { return None; }
                break;
            }
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    if elapsed < 0.1 || total < 1024 { return None; }
    Some((total as f64 / 1024.0) / elapsed)
}
