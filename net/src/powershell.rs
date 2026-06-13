//! PowerShell 下载回退 — 使用 Windows 原生 HTTP 栈（Schannel TLS）。
//!
//! 当所有其他策略（ureq/aria2c/curl）都被 CDN 拦截时，
//! PowerShell `Invoke-WebRequest` 使用 .NET 的 HttpClient，
//! 其 TLS 指纹（JA3）与 native-tls crate 不同，可能绕过 CDN 反爬。
//!
//! 此外 `Start-BitsTransfer` 使用 Windows BITS 服务，HTTP 栈更底层。

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context};

/// 使用 PowerShell `Invoke-WebRequest` 下载文件。
///
/// 参数：
/// - `url`: 下载地址
/// - `target_path`: 目标文件路径
pub fn try_powershell_download(url: &str, target_path: &Path) -> anyhow::Result<()> {
    // 确保目标目录存在
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!(
                "$cl = New-Object System.Net.WebClient; \
                 $cl.Headers.Add('User-Agent', 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'); \
                 $cl.Headers.Add('Referer', 'https://sunlogin.oray.com/'); \
                 $cl.Headers.Add('Accept', '*/*'); \
                 try {{ \
                   $cl.DownloadFile('{}', '{}'); \
                   exit 0 \
                 }} catch {{ \
                   exit 1 \
                 }}",
                url.replace('\'', "''"),
                target_path.to_string_lossy().replace('\'', "''")
            ),
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("运行 PowerShell 失败")?;

    if !status.success() {
        let _ = std::fs::remove_file(target_path);
        bail!("PowerShell 下载失败");
    }

    // 验证文件
    if !target_path.is_file() || std::fs::metadata(target_path).map(|m| m.len()).unwrap_or(0) == 0 {
        let _ = std::fs::remove_file(target_path);
        bail!("PowerShell 下载的文件为空或不存在");
    }

    Ok(())
}

/// 使用 PowerShell `Invoke-WebRequest` 下载（带更多浏览器头）。
///
/// 当基本 `WebClient` 也被拦截时，使用更完整的请求头。
pub fn try_powershell_invoke(url: &str, target_path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let ps_code = format!(
        "$headers = @{{ \
           'User-Agent' = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'; \
           'Referer' = 'https://sunlogin.oray.com/'; \
           'Accept' = '*/*'; \
           'Accept-Language' = 'zh-CN,zh;q=0.9'; \
           'Cache-Control' = 'no-cache' \
         }}; \
         try {{ \
           Invoke-WebRequest -Uri '{}' -OutFile '{}' -Headers $headers -UseBasicParsing -ErrorAction Stop; \
           exit 0 \
         }} catch {{ \
           exit 1 \
         }}",
        url.replace('\'', "''"),
        target_path.to_string_lossy().replace('\'', "''")
    );

    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &ps_code,
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("运行 PowerShell Invoke-WebRequest 失败")?;

    if !status.success() {
        let _ = std::fs::remove_file(target_path);
        bail!("PowerShell Invoke-WebRequest 下载失败");
    }

    if !target_path.is_file() || std::fs::metadata(target_path).map(|m| m.len()).unwrap_or(0) == 0 {
        let _ = std::fs::remove_file(target_path);
        bail!("PowerShell Invoke-WebRequest 下载的文件为空或不存在");
    }

    Ok(())
}

/// 使用 BITS (Background Intelligent Transfer Service) 下载。
///
/// BITS 是 Windows 自带的系统级下载服务，运行在 SVCHOST 进程中，
/// 使用 WinHTTP 栈。TLS 指纹与所有用户态库都不同。
pub fn try_bits_transfer(url: &str, target_path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let _job_name = format!("aminos_dl_{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0));

    // Start-BitsTransfer 创建后台任务，需要等待完成
    let ps_code = format!(
        "try {{ \
           $job = Start-BitsTransfer -Source '{}' -Destination '{}' -Asynchronous; \
           do {{ Start-Sleep -Seconds 1 }} while ($job.JobState -eq 'Transferring' -or $job.JobState -eq 'Connecting'); \
           if ($job.JobState -eq 'Transferred') {{ Complete-BitsTransfer -BitsJob $job; exit 0 }} \
           else {{ $job | Remove-BitsTransfer; exit 1 }} \
         }} catch {{ exit 1 }}",
        url.replace('\'', "''"),
        target_path.to_string_lossy().replace('\'', "''")
    );

    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &ps_code,
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("运行 BITS 下载失败")?;

    if !status.success() {
        let _ = std::fs::remove_file(target_path);
        bail!("BITS 下载失败");
    }

    if !target_path.is_file() || std::fs::metadata(target_path).map(|m| m.len()).unwrap_or(0) == 0 {
        let _ = std::fs::remove_file(target_path);
        bail!("BITS 下载的文件为空或不存在");
    }

    Ok(())
}
