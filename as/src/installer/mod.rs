mod portable;
mod installer_type;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use color::*;
use crate::software;
use crate::paths;

pub use portable::install_portable;
pub use installer_type::install_installer;

// ── 下载 ──────────────────────────────────────────

fn download_file(url: &str, target: &Path) -> anyhow::Result<()> {
    use net::download::DownloadConfig;

    println!("  下载: {}", cyan(url));
    let parent = target.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let config = DownloadConfig::default();
    net::download::download_with_url_fallback("", &[url.to_string()], target, &config)
        .with_context(|| format!("下载失败: {}", url))?;
    Ok(())
}

/// 获取下载文件路径（自动创建下载目录）
fn get_download_path(name: &str, ver: &str, url: &str) -> PathBuf {
    let filename = url.split('/').last()
        .or_else(|| url.split('\\').last())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}-{}.exe", name, ver));

    paths::downloads_dir().join(format!("{}-{}", name, ver)).join(filename)
}

/// 通过 URL 直接安装（外部链接安装）
pub fn install_from_url(url: &str) -> anyhow::Result<()> {
    let filename = url.split('/').last().unwrap_or("download.exe");
    let path = paths::downloads_dir().join(filename);
    download_file(url, &path)?;
    run_silent(&path, &["/S", "/VERYSILENT", "/SUPPRESSMSGBOXES"])?;
    Ok(())
}

/// 运行安装程序
fn run_silent(path: &Path, args: &[&str]) -> anyhow::Result<()> {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "msi" => {
            let status = std::process::Command::new("msiexec")
                .args(["/i", &path.to_string_lossy(), "/quiet", "/norestart"])
                .status()?;
            if !status.success() {
                anyhow::bail!("MSI 安装退出码: {:?}", status.code());
            }
        }
        _ => {
            // exe
            let status = std::process::Command::new(&path)
                .args(args)
                .status()?;
            if !status.success() {
                anyhow::bail!("安装程序退出码: {:?}", status.code());
            }
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn detect_type(path: &Path) -> &'static str {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "zip" => "zip",
        "7z" => "7z",
        "exe" | "msi" => "exe",
        _ => "unknown",
    }
}

/// 创建快捷桩
fn create_shim(name: &str, exe_path: &Path) {
    use std::fs;
    let bin_dir = paths::tools_bin_dir();
    fs::create_dir_all(&bin_dir).ok();

    let shim_path = bin_dir.join(format!("{}.cmd", name));
    let exe_abs = fs::canonicalize(exe_path)
        .unwrap_or_else(|_| exe_path.to_path_buf())
        .to_string_lossy()
        .to_string();

    let content = format!(
        "@echo off\nstart \"\" \"{}\" %*\n",
        exe_abs
    );
    fs::write(&shim_path, content).ok();
    println!("  创建快捷桩: {}", shim_path.display());
}

/// 记录安装
fn record_installation(name: &str, ver: &str, inst_type: &str, install_path: &str) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default();

    let record = software::InstallRecord {
        version: ver.to_string(),
        r#type: inst_type.to_string(),
        install_path: install_path.to_string(),
        install_time: now,
    };
    software::add_installed(name, record).ok();
}

/// 卸载已安装的软件
/// 解压 zip 文件到目标目录
pub fn extract_zip_to(archive_path: &Path, target_dir: &Path) -> anyhow::Result<()> {
    use std::fs;
    use anyhow::bail;

    fs::create_dir_all(target_dir)?;

    let status = std::process::Command::new("powershell")
        .args([
            "-NoProfile", "-Command",
            &format!(
                "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                archive_path.display(),
                target_dir.display()
            ),
        ])
        .status()?;

    if !status.success() {
        bail!("解压 zip 失败: {}", archive_path.display());
    }
    println!("  已解压到: {}", target_dir.display());
    Ok(())
}

pub fn uninstall_software(name: &str, rec: &software::InstallRecord) -> anyhow::Result<()> {
    match rec.r#type.as_str() {
        "portable" => {
            let target_dir = &rec.install_path;
            if Path::new(target_dir).exists() {
                std::fs::remove_dir_all(target_dir)?;
                println!("  已删除: {}", target_dir);
            }
            // 删除快捷桩
            let shim = paths::tools_bin_dir().join(format!("{}.cmd", name));
            if shim.exists() {
                std::fs::remove_file(&shim)?;
                println!("  已删除快捷桩: {}", shim.display());
            }
        }
        _ => {
            // installer 类型：尝试用注册表卸载
            uninstall_via_registry(name)?;
        }
    }
    Ok(())
}

/// 通过注册表卸载软件
pub fn uninstall_via_registry(name: &str) -> anyhow::Result<()> {
    let all = sys::registry::scan_all_installed_unfiltered();
    let info = all.iter().find(|m| {
        m.get("DisplayName").map(|n| n.to_lowercase() == name.to_lowercase()).unwrap_or(false)
    });

    if let Some(info) = info {
        if let Some(uninstall) = info.get("UninstallString") {
            println!("  执行卸载: {}", uninstall);
            // 提取实际路径（用于调试）
            let _clean = uninstall
                .trim_matches('"')
                .trim_start_matches("MsiExec.exe /I{")
                .trim_end_matches('}');
            let status = std::process::Command::new("cmd")
                .args(["/C", uninstall])
                .status()?;
            if status.success() {
                println!("  {} 卸载完成", color::bold_green("完成"));
                return Ok(());
            }
        }
        let dn = info.get("DisplayName").cloned().unwrap_or_default();
        println!("  找到 '{}' 但无卸载命令", dn);
    } else {
        eprintln!("  在注册表中未找到 '{}'", name);
    }
    Ok(())
}
