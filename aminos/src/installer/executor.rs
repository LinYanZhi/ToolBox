use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::bail;

use crate::paths;

use super::detect::detect_installer_type;

/// 安装器进程的最大等待时间（秒）。
/// 超时后若检测到软件已安装，则视为成功并强制结束进程。
const INSTALLER_TIMEOUT_SECS: u64 = 180;

/// 执行安装器。
///
/// 便携版：直接解压，返回解压后的路径。
/// 安装版：运行安装器（静默或 GUI），不返回路径。
///
/// 为防止安装器进程挂死不退出，引入超时机制：
/// 超过 `INSTALLER_TIMEOUT_SECS` 后若进程仍未退出，会检查软件是否已安装（通过注册表检测），
/// 若已安装则强制结束进程并视为安装成功；否则继续等待。
pub(crate) fn run_installer(
    name: &str,
    version: &str,
    installer_path: &Path,
    vi: &crate::software::VersionInfo,
    gui: bool,
) -> anyhow::Result<(bool, Option<PathBuf>)> {
    let itype = if vi.installer_type.is_empty() {
        detect_installer_type(installer_path)
    } else {
        &vi.installer_type
    };

    // Portable mode: extract archive
    if itype == "portable" {
        let path = install_portable(name, version, installer_path)?;
        return Ok((true, Some(path)));
    }

    // Build command
    let mut cmd = Command::new(installer_path);
    if !gui {
        for arg in &vi.install_args {
            cmd.arg(arg);
        }
    }

    if gui {
        println!("  以交互界面模式启动安装器");
    } else {
        println!("  静默安装 {} ...", itype);
    }

    // 使用 spawn + try_wait 轮询以支持超时
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("  启动安装程序失败: {}", e);
            return Ok((false, None));
        }
    };

    let deadline = Instant::now() + Duration::from_secs(INSTALLER_TIMEOUT_SECS);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // 进程正常退出
                if status.success() {
                    return Ok((true, None));
                }
                let code = status.code().unwrap_or(-1);
                if code == 1223 || code == 740 {
                    // 需要管理员权限 → 杀掉当前进程，尝试提权
                    let _ = child.kill();
                    let _ = child.wait();
                    println!("  需要管理员权限，尝试提权...");
                    return try_elevate(installer_path, &vi.install_args);
                }
                eprintln!("  安装程序返回错误码 {}", code);
                return Ok((false, None));
            }
            Ok(None) => {
                // 进程仍在运行
                if Instant::now() >= deadline {
                    // 超时：检查软件是否已安装
                    if let Some(ref detection) = vi.detection {
                        if crate::registry::detect_installed(detection).is_some() {
                            println!("  安装器进程超时（{} 秒），但软件已安装，强制结束进程...", INSTALLER_TIMEOUT_SECS);
                            let _ = child.kill();
                            let _ = child.wait();
                            return Ok((true, None));
                        }
                    }
                    // 没有 detection 或未检测到安装 → 阻塞等待（部分安装器安装很久才退出）
                    println!("  安装器已运行超过 {} 秒，仍在等待进程退出...", INSTALLER_TIMEOUT_SECS);
                    let status = child.wait();
                    match status {
                        Ok(s) if s.success() => return Ok((true, None)),
                        Ok(s) => {
                            eprintln!("  安装程序返回错误码 {}", s.code().unwrap_or(-1));
                            return Ok((false, None));
                        }
                        Err(e) => {
                            eprintln!("  等待安装进程失败: {}", e);
                            return Ok((false, None));
                        }
                    }
                } else {
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
            Err(e) => {
                eprintln!("  监控安装进程出错: {}", e);
                let _ = child.wait();
                return Ok((true, None));
            }
        }
    }
}

/// 安装便携版（第三方软件）。
pub(crate) fn install_portable(name: &str, version: &str, archive_path: &Path) -> anyhow::Result<PathBuf> {
    let dir_name = format!("{}-{}", name, version);
    let target = paths::apps_dir().join(&dir_name);
    if target.exists() {
        bail!("便携版目录已存在: {}", target.display());
    }

    let staging = target.with_extension("staging");
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;

    let ext = archive_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    println!("  解压中 ...");

    match ext.to_lowercase().as_str() {
        "zip" => {
            let status = Command::new("powershell")
                .args([
                    "-NoProfile", "-Command",
                    &format!(
                        "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                        archive_path.display(), staging.display()
                    ),
                ])
                .status()?;
            if !status.success() {
                bail!("解压 zip 失败");
            }
        }
        _ => {
            let candidates = [
                paths::builds_dir().join("7zr").join("7zr.exe"),
                paths::builds_dir().join("7zr.exe"),
                PathBuf::from("C:\\Program Files\\7-Zip\\7z.exe"),
                PathBuf::from("C:\\Program Files (x86)\\7-Zip\\7z.exe"),
            ];
            let seven_z = candidates.iter().find(|p| p.exists());
            let status = if let Some(exe) = seven_z {
                Command::new(exe)
                    .args(["x", &archive_path.to_string_lossy(), &format!("-o{}", staging.display()), "-y"])
                    .status()?
            } else {
                bail!("不支持的压缩格式 '{}'（未找到解压工具）。\n  提示：请安装 7-Zip 或将 7zr.exe 放入 {}",
                    ext, paths::builds_dir().display())
            };
            if !status.success() {
                bail!("解压失败");
            }
        }
    }

    let entries: Vec<_> = fs::read_dir(&staging)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            !name.starts_with('.') && !name.starts_with("__MACOSX")
        })
        .collect();

    if entries.is_empty() {
        fs::remove_dir(&staging)?;
        bail!("压缩包为空或仅包含系统文件");
    }

    if entries.len() == 1 && entries[0].file_type().map(|t| t.is_dir()).unwrap_or(false) {
        let single_dir = entries[0].path();
        fs::rename(&single_dir, &target)?;
    } else {
        fs::create_dir(&target)?;
        for entry in &entries {
            let src = entry.path();
            let dest = target.join(entry.file_name());
            fs::rename(&src, &dest)?;
        }
    }

    let _ = fs::remove_dir(&staging);
    println!("  已解压到 {}", target.display());
    Ok(target)
}

/// 将压缩包解压到指定目录（自研工具用）。
/// 如果压缩包内只有一个根目录，则提取该目录的内容；否则直接解压到目标目录。
pub fn extract_zip_to(archive_path: &Path, target_dir: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(target_dir)?;

    let ext = archive_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let staging = target_dir.with_extension("staging");
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;

    match ext.as_str() {
        "zip" => {
            let status = Command::new("powershell")
                .args([
                    "-NoProfile", "-Command",
                    &format!(
                        "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                        archive_path.display(), staging.display()
                    ),
                ])
                .status()?;
            if !status.success() {
                bail!("解压 zip 失败");
            }
        }
        _ => {
            let candidates = [
                paths::builds_dir().join("7zr").join("7zr.exe"),
                paths::builds_dir().join("7zr.exe"),
                PathBuf::from("C:\\Program Files\\7-Zip\\7z.exe"),
                PathBuf::from("C:\\Program Files (x86)\\7-Zip\\7z.exe"),
            ];
            let seven_z = candidates.iter().find(|p| p.exists());
            let status = if let Some(exe) = seven_z {
                Command::new(exe)
                    .args(["x", &archive_path.to_string_lossy(), &format!("-o{}", staging.display()), "-y"])
                    .status()?
            } else {
                bail!("不支持的压缩格式 '{}'（未找到解压工具）", ext);
            };
            if !status.success() {
                bail!("解压失败");
            }
        }
    }

    // 检查 staging 是否只有一个根目录
    let entries: Vec<_> = fs::read_dir(&staging)?
        .filter_map(|e| e.ok())
        .collect();

    if entries.len() == 1 && entries[0].file_type().map(|t| t.is_dir()).unwrap_or(false) {
        let inner = entries[0].path();
        for entry in fs::read_dir(&inner)? {
            let entry = entry?;
            let target = target_dir.join(entry.file_name());
            let _ = fs::remove_dir_all(&target);
            fs::rename(entry.path(), &target)?;
        }
    } else {
        for entry in entries {
            let target = target_dir.join(entry.file_name());
            let _ = fs::remove_dir_all(&target);
            fs::rename(entry.path(), &target)?;
        }
    }

    let _ = fs::remove_dir_all(&staging);
    Ok(())
}

/// 通过 PowerShell UAC 提权运行安装器。
pub(crate) fn try_elevate(installer_path: &Path, args: &[String]) -> anyhow::Result<(bool, Option<PathBuf>)> {
    let mut ps_args = format!(
        "Start-Process -FilePath '{}'",
        installer_path.display()
    );
    if !args.is_empty() {
        let arg_str = args.iter()
            .map(|a| format!("'{}'", a.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(", ");
        ps_args.push_str(&format!(" -ArgumentList {}", arg_str));
    }
    ps_args.push_str(" -Verb RunAs -Wait");

    let status = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_args])
        .status()?;

    if !status.success() {
        eprintln!("  UAC 提权被取消或失败");
    }
    Ok((status.success(), None))
}
