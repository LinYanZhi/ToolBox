//! PowerShell 下载回退 — 使用 Windows 原生 HTTP 栈（Schannel TLS）。
//!
//! 当所有其他策略（ureq/aria2c/curl）都被 CDN 拦截时，
//! PowerShell `Invoke-WebRequest` 使用 .NET 的 HttpClient，
//! 其 TLS 指纹（JA3）与 native-tls crate 不同，可能绕过 CDN 反爬。
//!
//! 此外 `Start-BitsTransfer` 使用 Windows BITS 服务，HTTP 栈更底层。

use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context};

use crate::download::Cancel;

/// 使用 PowerShell `Invoke-WebRequest` 下载文件。
/// 支持通过 Cancel 取消并自动清理临时文件。
pub fn try_powershell_download(url: &str, target_path: &Path, cancel: &Cancel) -> anyhow::Result<()> {
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut child = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!(
                "$ProgressPreference = 'SilentlyContinue'; \
$cl = New-Object System.Net.WebClient; \
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
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("启动 PowerShell 失败")?;

    // 自定义等待循环：取消时先 kill PowerShell，再清理 BITS 孤儿任务
    loop {
        if cancel.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            // 清理 BITS 孤儿任务（PowerShell 被 kill 后 BITS 作业可能残留）
            let _ = std::process::Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command",
                    "Get-BitsTransfer | Remove-BitsTransfer -Confirm:$false"])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            thread::sleep(Duration::from_millis(500)); // 等 BITS 释放文件锁
            let _ = std::fs::remove_file(target_path);
            return Err(anyhow::anyhow!("已取消"));
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if cancel.is_cancelled() {
                    let _ = std::fs::remove_file(target_path);
                    return Err(anyhow::anyhow!("已取消"));
                }
                if !status.success() {
                    let _ = std::fs::remove_file(target_path);
                    return Err(anyhow::anyhow!("进程退出码 {}", status.code().unwrap_or(-1)));
                }
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(_) => {
                let _ = std::fs::remove_file(target_path);
                bail!("等待进程退出失败");
            }
        }
    }

    if !target_path.is_file() || std::fs::metadata(target_path).map(|m| m.len()).unwrap_or(0) == 0 {
        let _ = std::fs::remove_file(target_path);
        bail!("PowerShell 下载的文件为空或不存在");
    }
    Ok(())
}

/// 使用 PowerShell `Invoke-WebRequest` 下载（带更多浏览器头）。
pub fn try_powershell_invoke(url: &str, target_path: &Path, cancel: &Cancel) -> anyhow::Result<()> {
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let ps_code = format!(
        "$ProgressPreference = 'SilentlyContinue'; \
$headers = @{{ \
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

    let mut child = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_code])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("启动 PowerShell Invoke-WebRequest 失败")?;

    // BITS 专用等待循环：取消时清理 BITS 孤儿作业，避免 BIT*.tmp 残留
    loop {
        if cancel.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            // 清理 BITS 孤儿任务
            let _ = std::process::Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command",
                    "Get-BitsTransfer | Remove-BitsTransfer -Confirm:$false"])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            thread::sleep(Duration::from_millis(500));
            let _ = std::fs::remove_file(target_path);
            return Err(anyhow::anyhow!("已取消"));
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if cancel.is_cancelled() {
                    let _ = std::fs::remove_file(target_path);
                    return Err(anyhow::anyhow!("已取消"));
                }
                if !status.success() {
                    let _ = std::fs::remove_file(target_path);
                    return Err(anyhow::anyhow!("进程退出码 {}", status.code().unwrap_or(-1)));
                }
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(_) => {
                let _ = std::fs::remove_file(target_path);
                bail!("等待进程退出失败");
            }
        }
    }

    if !target_path.is_file() || std::fs::metadata(target_path).map(|m| m.len()).unwrap_or(0) == 0 {
        let _ = std::fs::remove_file(target_path);
        bail!("PowerShell Invoke-WebRequest 下载的文件为空或不存在");
    }
    Ok(())
}

/// 使用 BITS (Background Intelligent Transfer Service) 下载。
pub fn try_bits_transfer(url: &str, target_path: &Path, cancel: &Cancel) -> anyhow::Result<()> {
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let ps_code = format!(
        "$ProgressPreference = 'SilentlyContinue'; \
try {{ \
  $job = Start-BitsTransfer -Source '{}' -Destination '{}' -Asynchronous; \
  do {{ Start-Sleep -Seconds 1 }} while ($job.JobState -eq 'Transferring' -or $job.JobState -eq 'Connecting'); \
  if ($job.JobState -eq 'Transferred') {{ Complete-BitsTransfer -BitsJob $job; exit 0 }} \
  else {{ $job | Remove-BitsTransfer; exit 1 }} \
}} catch {{ exit 1 }}",
        url.replace('\'', "''"),
        target_path.to_string_lossy().replace('\'', "''")
    );

    let mut child = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_code])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("启动 BITS 下载失败")?;

    // BITS 专用等待循环：取消时清理 BITS 孤儿作业
    loop {
        if cancel.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::process::Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command",
                    "Get-BitsTransfer | Remove-BitsTransfer -Confirm:$false"])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            thread::sleep(Duration::from_millis(500));
            let _ = std::fs::remove_file(target_path);
            return Err(anyhow::anyhow!("已取消"));
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if cancel.is_cancelled() {
                    let _ = std::fs::remove_file(target_path);
                    return Err(anyhow::anyhow!("已取消"));
                }
                if !status.success() {
                    let _ = std::fs::remove_file(target_path);
                    return Err(anyhow::anyhow!("进程退出码 {}", status.code().unwrap_or(-1)));
                }
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(_) => {
                let _ = std::fs::remove_file(target_path);
                bail!("等待进程退出失败");
            }
        }
    }

    if !target_path.is_file() || std::fs::metadata(target_path).map(|m| m.len()).unwrap_or(0) == 0 {
        let _ = std::fs::remove_file(target_path);
        bail!("BITS 下载的文件为空或不存在");
    }
    Ok(())
}
