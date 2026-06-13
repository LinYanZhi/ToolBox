//! 下载器兼容层 — 委托到 `net` crate。
//!
//! 保留 `aminos` 原有的公共 API 签名，避免一次性修改所有调用方。
//! 新代码应直接使用 `net::*`。

use std::path::Path;

use anyhow::Result;

pub use net::format_size;
pub use net::verify_downloaded_file;

// Re-export color utilities
pub fn display_width(s: &str) -> usize {
    use color::DisplayWidth;
    s.display_width()
}

pub fn pad(s: &str, width: usize) -> String {
    color::pad_left(s, width)
}

/// 多策略下载（兼容旧签名）。
#[allow(dead_code)]
pub fn download_with_progress(url: &str, target_path: &Path, renew: bool) -> Result<()> {
    let config = net::DownloadConfig::default().renew(renew);
    net::download_with_fallback(url, target_path, &config)?;
    Ok(())
}

/// URL 回退下载（兼容旧签名）。
pub fn download_with_fallback(
    name: &str,
    urls: &[String],
    target_path: &Path,
    renew: bool,
) -> Result<usize> {
    let config = net::DownloadConfig::default().renew(renew);
    net::download::download_with_url_fallback(name, urls, target_path, &config)?;
    Ok(0) // 旧签名返回 index，新签名用 DownloadReport
}

/// 构造安全的安装包文件名。
pub fn safe_installer_name(name: &str, version: &str, urls: &[String]) -> String {
    let safe_name = name.to_lowercase().replace(' ', "-");
    let safe_ver = version.to_lowercase().replace(' ', "-");

    if let Some(first_url) = urls.first() {
        let path = first_url.split('?').next().unwrap_or(first_url);
        let seg = path.rsplit('/').next().unwrap_or("");
        if let Some(dot) = seg.rfind('.') {
            let e = &seg[dot..];
            if [
                ".exe", ".msi", ".zip", ".7z", ".rar", ".tar", ".gz", ".xz", ".bz2", ".iso",
                ".appx", ".dmg",
            ]
            .contains(&e.to_lowercase().as_str())
            {
                return format!("{}-{}{}", safe_name, safe_ver, e);
            }
        }
    }
    format!("{}-{}.exe", safe_name, safe_ver)
}

/// 对 GitHub 地址自动追加 ghproxy 镜像。
pub fn expand_github_urls(urls: &[String]) -> Vec<String> {
    net::download::expand_github_urls(urls)
}

/// 测量下载速度（KB/s）。
pub fn measure_speed(url: &str, timeout_secs: u64) -> Option<f64> {
    net::speedtest::measure_speed(url, timeout_secs)
}

/// CJK 感知的截断（已迁移到 color crate）。
pub fn truncate_display(s: &str, max_width: usize) -> String {
    color::truncate(s, max_width)
}
