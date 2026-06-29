use std::path::{Path, PathBuf};
use std::fs;
use std::io::{Read, Write};
use anyhow::Context;

pub fn install_installer(
    name: &str,
    _version: &str,
    dl_path: &Path,
    detect: Option<&crate::software::DetectConfig>,
) -> anyhow::Result<String> {
    let ext = dl_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let mut child = match ext.as_str() {
        "msi" => {
            println!("  正在启动 MSI 安装程序（请在弹出的窗口中完成安装）...");
            std::process::Command::new("msiexec")
                .args(["/i", &dl_path.to_string_lossy()])
                .spawn()
                .context("启动 msiexec 失败")?
        }
        "exe" => {
            println!("  正在启动安装程序（请在弹出的窗口中完成安装）...");
            std::process::Command::new(dl_path)
                .spawn()
                .context("启动安装程序失败")?
        }
        _ => {
            let real = detect_archive_type(dl_path);
            match real {
                "single" => {
                    let exe_path = dl_path.with_extension("exe");
                    let _ = std::fs::rename(dl_path, &exe_path);
                    return install_installer(name, _version, &exe_path, detect);
                }
                "zip" | "7z" => {
                    return install_compressed_installer(name, _version, dl_path, real, detect);
                }
                "tar" => {
                    return install_tar_installer(name, _version, dl_path, detect);
                }
                _ => anyhow::bail!("不支持的安装包类型"),
            }
        }
    };

    print!("  等待安装完成（安装程序关闭后将自动继续）");
    let _ = std::io::stdout().flush();
    for i in 0..150 {
        match child.try_wait() {
            Ok(Some(status)) => {
                println!();
                println!("  安装程序已退出 (退出码: {:?})", status.code());
                if let Some(path) = check_registry(name, detect, dl_path) {
                    return Ok(path);
                }
                println!("  {} 注册表中未检测到 {} 的安装记录", color::yellow("提示"), name);
                println!("    请手动确认安装是否已完成");
                return Ok("(用户确认)".to_string());
            }
            Ok(None) => {}
            Err(_) => {}
        }

        if let Some(path) = check_registry(name, detect, dl_path) {
            println!();
            return Ok(path);
        }

        if i > 0 && i % 15 == 0 {
            let secs = (i + 1) * 2;
            print!(" 已等待 {} 秒", secs);
        } else {
            print!(".");
        }
        let _ = std::io::stdout().flush();
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    anyhow::bail!(
        "等待注册超时（已等待 5 分钟），注册表中未检测到 {}",
        name
    );
}

fn check_registry(
    name: &str,
    detect: Option<&crate::software::DetectConfig>,
    dl_path: &Path,
) -> Option<String> {
    if let Some(detect_cfg) = detect {
        crate::software::detect_from_registry(detect_cfg)
            .map(|info| {
                let install_path = info.install_path.unwrap_or_else(|| {
                    format!("{} (系统程序)", dl_path.to_string_lossy())
                });
                if !info.version.is_empty() {
                    println!("  检测到已安装版本: {}", info.version);
                }
                install_path
            })
    } else {
        None
    }
}

fn install_tar_installer(
    name: &str,
    _version: &str,
    dl_path: &Path,
    detect: Option<&crate::software::DetectConfig>,
) -> anyhow::Result<String> {
    let extract_dir = dl_path.parent().unwrap_or(Path::new(".")).join(format!("{}_extracted", name));
    let _ = std::fs::remove_dir_all(&extract_dir);
    std::fs::create_dir_all(&extract_dir)?;

    println!("  解压 tar 包...");
    let status = std::process::Command::new("tar")
        .args(["-xf", &dl_path.to_string_lossy(), "-C", &extract_dir.to_string_lossy()])
        .status()
        .map_err(|e| anyhow::anyhow!("tar.exe 解压失败: {}", e))?;

    if !status.success() {
        let _ = std::fs::remove_dir_all(&extract_dir);
        anyhow::bail!("tar 解压失败");
    }

    let exe = find_exe_recursive(&extract_dir).ok_or_else(|| {
        let _ = std::fs::remove_dir_all(&extract_dir);
        anyhow::anyhow!("tar 包内未找到 exe 安装程序")
    })?;

    println!("  找到安装程序: {}", exe.file_name().unwrap_or_default().to_string_lossy());

    let target_exe = dl_path.parent().unwrap_or(Path::new(".")).join(
        exe.file_name().unwrap_or_default()
    );
    std::fs::copy(&exe, &target_exe)?;

    let _ = std::fs::remove_dir_all(&extract_dir);

    install_installer(name, _version, &target_exe, detect)
}

fn install_compressed_installer(
    name: &str,
    _version: &str,
    dl_path: &Path,
    archive_type: &str,
    detect: Option<&crate::software::DetectConfig>,
) -> anyhow::Result<String> {
    let extract_dir = dl_path.parent().unwrap_or(Path::new(".")).join(format!("{}_extracted", name));
    let _ = std::fs::remove_dir_all(&extract_dir);
    std::fs::create_dir_all(&extract_dir)?;

    if archive_type == "zip" {
        extract_zip(dl_path, &extract_dir)?;
    } else if archive_type == "7z" {
        println!("  解压 7z 包...");
        let status = std::process::Command::new("7z")
            .args(["x", &dl_path.to_string_lossy(), &format!("-o{}", extract_dir.to_string_lossy()), "-y"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| anyhow::anyhow!("7z 解压失败: {}", e))?;

        if !status.success() {
            let _ = std::fs::remove_dir_all(&extract_dir);
            anyhow::bail!("7z 解压失败");
        }
    }

    let exe = find_exe_recursive(&extract_dir).ok_or_else(|| {
        let _ = std::fs::remove_dir_all(&extract_dir);
        anyhow::anyhow!("压缩包内未找到 exe 安装程序")
    })?;

    println!("  找到安装程序: {}", exe.file_name().unwrap_or_default().to_string_lossy());

    let result = install_installer(name, _version, &exe, detect);

    let _ = std::fs::remove_dir_all(&extract_dir);

    result
}

fn find_exe_recursive(dir: &Path) -> Option<std::path::PathBuf> {
    if dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        let ext_lower = ext.to_str()?.to_lowercase();
                        if ext_lower == "msi" {
                            return Some(path);
                        }
                        if ext_lower == "exe" {
                            if let Some(stem) = path.file_stem()?.to_str() {
                                let stem_lower = stem.to_lowercase();
                                if stem_lower == "setup"
                                    || stem_lower == "install"
                                    || stem_lower.starts_with("vcredist")
                                    || stem_lower.starts_with("vc_redist")
                                    || stem_lower.starts_with("dotnet")
                                    || stem_lower.ends_with("-amd64")
                                    || stem_lower.ends_with("-x64")
                                    || stem_lower.ends_with("-x86")
                                {
                                    return Some(path);
                                }
                            }
                        }
                    }
                } else if path.is_dir() {
                    if let Some(found) = find_exe_recursive(&path) {
                        return Some(found);
                    }
                }
            }
        }
    }
    None
}

fn extract_zip(zip_path: &Path, target: &Path) -> anyhow::Result<()> {
    let zip_str = zip_path.to_string_lossy();
    let target_str = target.to_string_lossy();

    println!("  解压 {} -> {}", color::gray(&zip_str), color::gray(&target_str));

    let ok1 = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                zip_str, target_str
            ),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if ok1 && is_nonempty_dir(target) {
        return Ok(());
    }

    let _ = fs::remove_dir_all(target);
    let _ = fs::create_dir_all(target);
    let ok2 = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Add-Type -AssemblyName System.IO.Compression.FileSystem; \
                 [System.IO.Compression.ZipFile]::ExtractToDirectory('{}', '{}')",
                zip_str, target_str
            ),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if ok2 && is_nonempty_dir(target) {
        return Ok(());
    }

    anyhow::bail!("解压失败：无法解压 {}", zip_str)
}

fn is_nonempty_dir(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    match dir.read_dir() {
        Ok(mut iter) => iter.next().is_some(),
        Err(_) => false,
    }
}

fn extract_7z(path: &Path, target: &Path) -> anyhow::Result<()> {
    if let Ok(status) = std::process::Command::new("7z")
        .args(["x", &path.to_string_lossy(), &format!("-o{}", target.to_string_lossy()), "-y"])
        .status()
    {
        if status.success() {
            return Ok(());
        }
    }

    let seven_zip = Path::new(r"C:\Program Files\7-Zip\7z.exe");
    if seven_zip.is_file() {
        if let Ok(status) = std::process::Command::new(seven_zip)
            .args(["x", &path.to_string_lossy(), &format!("-o{}", target.to_string_lossy()), "-y"])
            .status()
        {
            if status.success() {
                return Ok(());
            }
        }
    }

    anyhow::bail!("无法解压 .7z 文件，请安装 7-Zip")
}

fn detect_archive_type(path: &Path) -> &'static str {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "zip" => return "zip",
        "7z" => return "7z",
        "tar" => return "tar",
        "exe" | "msi" => return "single",
        _ => {}
    }

    let mut buf = [0u8; 8];
    if let Ok(mut f) = fs::File::open(path) {
        if f.read_exact(&mut buf).is_ok() {
            if buf[..4] == [0x50, 0x4B, 0x03, 0x04] {
                return "zip";
            }
            if buf[..6] == [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C] {
                return "7z";
            }
            if buf[..2] == [0x4D, 0x5A] {
                return "single";
            }
        }
    }

    "single"
}

pub fn uninstall_installer(name: &str, detect: Option<&crate::software::DetectConfig>) -> anyhow::Result<()> {
    if let Some(detect_cfg) = detect {
        if let Some(info) = crate::software::detect_from_registry(detect_cfg) {
            if let Some(uninstall) = info.uninstall_string {
                println!("  执行卸载: {}", uninstall);
                let status = std::process::Command::new("cmd")
                    .args(["/C", &uninstall])
                    .status()?;
                if status.success() {
                    println!("  {} 卸载完成", color::bold_green("完成"));
                    return Ok(());
                }
            }
            let dn = info.display_name;
            println!("  找到 '{}' 但无卸载命令", dn);
        } else {
            eprintln!("  在注册表中未找到 '{}'", name);
        }
    } else {
        eprintln!("  {} 没有注册表检测配置，无法自动卸载", name);
    }
    Ok(())
}