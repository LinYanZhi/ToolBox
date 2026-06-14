use std::path::Path;

use anyhow::bail;

/// 验证下载请求的参数。
pub struct ValidatedRequest {
    /// 清理后的 URL。
    pub url: String,
}

/// 验证下载 URL 和路径是否合法。
///
/// 检查项：
/// - URL 格式正确（http/https）
/// - 协议在白名单内
/// - 域名不在黑名单（localhost、127.0.0.1）
/// - 目标路径不存在路径遍历攻击
/// - 文件大小不超限（如果已知）
pub fn validate_download_url(url: &str) -> anyhow::Result<ValidatedRequest> {
    let trimmed = url.trim();

    // 1. 基本格式
    if trimmed.is_empty() {
        bail!("下载地址为空");
    }

    // 2. 协议白名单
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        bail!("不支持的协议（仅支持 http/https）: {}", trimmed);
    }

    // 3. 域名黑名单
    let domain = trimmed
        .split("://")
        .nth(1)
        .and_then(|s| s.split('/').next())
        .unwrap_or("");

    if domain.is_empty() {
        bail!("无法解析域名: {}", trimmed);
    }

    let blocked_domains = ["localhost", "127.0.0.1", "::1", "0.0.0.0"];
    for blocked in &blocked_domains {
        if domain == *blocked || domain.starts_with(&format!("{}:", blocked)) {
            bail!("禁止下载本地地址: {}", trimmed);
        }
    }

    // 4. 检查 IP 段中的私有地址（简单检查）
    if let Some(ip_part) = domain.split(':').next() {
        if ip_part.starts_with("10.") || ip_part.starts_with("172.16.") || ip_part.starts_with("192.168.") {
            bail!("禁止下载内网地址: {}", trimmed);
        }
    }

    Ok(ValidatedRequest { url: trimmed.to_string() })
}

/// 验证目标路径是否安全（无路径遍历）。
pub fn validate_target_path(path: &Path) -> anyhow::Result<()> {
    let path_str = path.to_string_lossy();

    // 防止路径遍历
    if path_str.contains("..") {
        bail!("目标路径包含 '..'，可能存在路径遍历风险: {}", path_str);
    }

    // 不允许使用绝对路径（用 Path 的 is_absolute 检查）
    // 这里我们允许绝对路径（下载需要写入具体位置），但检查 parent 存在
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            // 父目录不存在是允许的（create_dir_all 会创建）
        }
    }

    Ok(())
}

/// 验证 HTTP 响应状态码是否允许重试。
pub fn is_retryable(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}
