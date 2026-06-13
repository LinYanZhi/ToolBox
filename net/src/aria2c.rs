use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context};

use crate::agent::Fingerprint;

/// 使用系统 aria2c 多线程下载。
///
/// aria2c 会分多线程下载，速度远快于单线程 HTTPS。
/// 自动续传未完成的 `.downloading` 文件。
pub fn try_aria2c_download(url: &str, target_path: &Path) -> anyhow::Result<()> {
    let aria2c = find_aria2c()
        .ok_or_else(|| anyhow::anyhow!("未找到 aria2c.exe"))?;

    let filename = target_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let parent = target_path.parent().unwrap_or(Path::new("."));

    // 检查续传
    let partial_path = format!("{}.downloading", target_path.to_string_lossy());
    let has_partial = Path::new(&partial_path).is_file();
    let target_exists = target_path.is_file();

    let mut cmd = Command::new(&aria2c);
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
    ]);

    if has_partial || target_exists {
        cmd.arg("--continue=true");
    }

    cmd.args([
        "--dir",
        &parent.to_string_lossy(),
        "--out",
        &filename,
    ]);

    // 传入浏览器模拟请求头（防盗链关键）
    // 使用保守的请求头，不发送 Sec-Fetch-* 等浏览器专属头（防反爬误判）
    cmd.arg("--header");
    cmd.arg(format!("User-Agent: {}", Fingerprint::Chrome120.user_agent()));
    cmd.arg("--header");
    cmd.arg("Accept: */*");
    cmd.arg("--header");
    cmd.arg("Accept-Language: zh-CN,zh;q=0.9,en;q=0.8");

    let hostname = url
        .split("://")
        .nth(1)
        .and_then(|s| s.split('/').next())
        .unwrap_or("");
    if !hostname.is_empty() {
        cmd.arg("--header");
        cmd.arg(format!("Referer: https://{}/", hostname));
    }

    cmd.arg(url);

    let status = cmd
        .stdin(std::process::Stdio::null())
        .status()
        .context("运行 aria2c 失败")?;

    if !status.success() {
        let _ = std::fs::remove_file(&partial_path);
        let _ = std::fs::remove_file(target_path);
        bail!("aria2c 进程异常退出");
    }

    if !target_path.is_file()
        || std::fs::metadata(target_path)
            .map(|m| m.len())
            .unwrap_or(0)
            == 0
    {
        let _ = std::fs::remove_file(&partial_path);
        let _ = std::fs::remove_file(target_path);
        bail!("aria2c 下载失败（文件不存在或为空）");
    }

    Ok(())
}

/// 查找 aria2c.exe。
///
/// 优先级：
///   1. 环境变量 `AMINOS_ARIA2C_PATH`
///   2. `%LOCALAPPDATA%\aminos\tools\aria2c\aria2c.exe`（as 工具包管理）
///   3. `%USERPROFILE%\Desktop`
///   4. PATH 环境变量
fn find_aria2c() -> Option<std::path::PathBuf> {
    // 1. 环境变量
    if let Ok(path) = std::env::var("AMINOS_ARIA2C_PATH") {
        let p = std::path::PathBuf::from(path);
        if p.is_file() {
            return Some(p);
        }
    }

    // 2. as 工具包目录：%LOCALAPPDATA%\aminos\tools\aria2c\aria2c.exe
    if let Some(localappdata) = std::env::var_os("LOCALAPPDATA") {
        let candidate = std::path::PathBuf::from(localappdata)
            .join("aminos")
            .join("tools")
            .join("aria2c")
            .join("aria2c.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    // 3. 桌面
    if let Some(desktop) = std::env::var_os("USERPROFILE")
        .map(|p| std::path::PathBuf::from(p).join("Desktop").join("aria2c.exe"))
    {
        if desktop.is_file() {
            return Some(desktop);
        }
    }

    // 4. PATH
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
