use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
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
    let exe = std::env::current_exe().ok()?;
    let aria2c = exe.parent()?.join("aria2c.exe");
    if aria2c.is_file() {
        Some(aria2c)
    } else {
        None
    }
}

/// 使用 aria2c 多线程下载（自动检测并启用）
///
/// aria2c 会分多线程下载，速度远快于单线程 HTTPS。
/// 参数: -x 16 最多 16 连接, -s 16 16 分块, --retry-wait 3 重试等待 3s
fn try_aria2c_download(url: &str, target_path: &Path) -> anyhow::Result<()> {
    let aria2c = find_aria2c()
        .ok_or_else(|| anyhow::anyhow!("未找到 aria2c.exe"))?;

    let filename = target_path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let parent = target_path.parent().unwrap_or(Path::new("."));

    println!("  使用 aria2c 多线程下载: {} ...", filename);

    let status = std::process::Command::new(&aria2c)
        .args([
            "-x", "16",           // 最多 16 连接
            "-s", "16",           // 16 分块
            "-k", "1M",           // 每块 1MB
            "--retry-wait", "3",  // 重试等待 3 秒
            "--max-tries", "5",   // 最多重试 5 次
            "--connect-timeout", "30",
            "--timeout", "600",
            "--allow-overwrite", "true",
            "--auto-file-renaming", "false",
            "--dir", &parent.to_string_lossy(),
            "--out", &filename,
            "--summary-interval", "0",
            url,
        ])
        .stdin(std::process::Stdio::null())
        .status()
        .context("运行 aria2c 失败")?;

    if !status.success() {
        let code = status.code().unwrap_or(-1);
        bail!("aria2c 退出码 {}", code);
    }

    Ok(())
}

/// Download a file from `url` to `target_path`, showing a progress bar.
///
/// Tries: normal TLS → insecure TLS → aria2c (if found) → system curl.exe.
pub fn download_with_progress(url: &str, target_path: &Path, renew: bool) -> anyhow::Result<()> {
    if target_path.exists() && !renew {
        println!("  使用缓存: {}", target_path.display());
        return Ok(());
    }

    // Tier 1: Normal TLS
    let r1 = with_browser_headers(normal_agent(30, 600).get(url), url).call();
    if let Ok(resp) = r1 {
        return download_body(resp, target_path);
    }
    let err_normal = r1.unwrap_err();

    // Tier 2: Insecure TLS
    match insecure_agent(30, 600) {
        Ok(agent) => {
            match with_browser_headers(agent.get(url), url).call() {
                Ok(resp) => return download_body(resp, target_path),
                Err(e) => {
                    // Tier 3: aria2c (多线程，与 as.exe 同目录时自动启用)
                    if find_aria2c().is_some() {
                        match try_aria2c_download(url, target_path) {
                            Ok(()) => return Ok(()),
                            Err(aria2_err) => {
                                // Tier 4: system curl
                                if let Err(e2) = try_curl_download(url, target_path) {
                                    bail!("无法下载 (TLS: {}; insecure TLS: {}; aria2c: {}; curl: {})",
                                        err_normal, e, aria2_err, e2);
                                }
                                return Ok(());
                            }
                        }
                    }
                    // Tier 3 (no aria2c): system curl
                    if let Err(e2) = try_curl_download(url, target_path) {
                        bail!("无法下载 (TLS: {}; insecure TLS: {}; curl: {})", err_normal, e, e2);
                    }
                    return Ok(());
                }
            }
        }
        Err(e) => {
            // If insecure agent fails, still try aria2c if available
            if find_aria2c().is_some() {
                match try_aria2c_download(url, target_path) {
                    Ok(()) => return Ok(()),
                    Err(aria2_err) => {
                        if let Err(e2) = try_curl_download(url, target_path) {
                            bail!("无法下载 (TLS: {}; insecure init: {}; aria2c: {}; curl: {})",
                                err_normal, e, aria2_err, e2);
                        }
                        return Ok(());
                    }
                }
            }
            if let Err(e2) = try_curl_download(url, target_path) {
                bail!("无法下载 (TLS: {}; insecure init: {}; curl: {})", err_normal, e, e2);
            }
            return Ok(());
        }
    }
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

    // 校验：检查文件签名是否匹配预期类型
    let fname_lower = filename.to_lowercase();
    let mut preview = String::new();
    let valid = validate_file_signature(target_path, &fname_lower, &mut preview);
    if !valid {
        let _ = fs::remove_file(target_path);
        anyhow::bail!("文件签名不匹配，下载到了非安装包内容（前 200 字节: {}）",
            preview.chars().filter(|c| c.is_ascii_graphic() || *c == ' ').take(80).collect::<String>());
    }

    Ok(())
}

/// 校验下载文件的签名是否合法
fn validate_file_signature(path: &Path, fname_lower: &str, preview: &mut String) -> bool {
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

    // 预览：把前 200 字节转成字符串（用于诊断报错）
    if let Ok(s) = String::from_utf8(header[..n.min(200)].to_vec()) {
        *preview = s;
    }

    // 按扩展名检查文件魔数
    if fname_lower.ends_with(".exe") || fname_lower.ends_with(".dll") || fname_lower.ends_with(".msi") {
        // PE 文件: MZ 开头 (4D 5A)
        header[0] == 0x4D && header[1] == 0x5A
    } else if fname_lower.ends_with(".zip") || fname_lower.ends_with(".7z") {
        // ZIP: PK\x03\x04, 7z: 37 7A BC AF 27 1C
        (header[0] == 0x50 && header[1] == 0x4B && header[2] == 0x03 && header[3] == 0x04)
            || (header[0] == 0x37 && header[1] == 0x7A)
    } else if fname_lower.ends_with(".rar") {
        header[0] == 0x52 && header[1] == 0x61 && header[2] == 0x72 && header[3] == 0x21
    } else if fname_lower.ends_with(".tar") {
        // tar 无固定魔数，用 n > 1KB 兜底
        n > 1024
    } else if fname_lower.ends_with(".gz") || fname_lower.ends_with(".xz") || fname_lower.ends_with(".bz2") {
        // gz: 1F 8B, xz: FD 37 7A 58 5A, bz2: 42 5A 68
        (header[0] == 0x1F && header[1] == 0x8B)
            || (header[0] == 0xFD && header[1] == 0x37)
            || (header[0] == 0x42 && header[1] == 0x5A)
    } else if fname_lower.ends_with(".iso") {
        // ISO: CD 00 1, or CD 01 1
        header[0] == 0x43 && header[1] == 0x44 && header[2] == 0x30
    } else if fname_lower.ends_with(".appx") || fname_lower.ends_with(".msix") {
        // APPX/MSIX: 也是 ZIP 格式 (PK\x03\x04)
        header[0] == 0x50 && header[1] == 0x4B && header[2] == 0x03 && header[3] == 0x04
    } else if fname_lower.ends_with(".dmg") {
        // DMG: 无法仅用前 4 字节判断，用 n > 1KB 兜底
        n > 1024
    } else {
        // 未知类型，信任大小
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
