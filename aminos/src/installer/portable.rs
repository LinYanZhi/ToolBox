use std::path::Path;
use std::fs;

use anyhow::Context;

use crate::installer::{download_file, get_download_path, create_shim, record_installation};
use crate::paths;

/// 安装便携版软件
pub fn install_portable(name: &str, ver: &str, urls: &[String]) -> anyhow::Result<()> {
    let url = &urls[0];
    let dl_path = get_download_path(name, ver, url);
    let dl_dir = dl_path.parent().unwrap();
    fs::create_dir_all(dl_dir)?;

    // 下载
    if !dl_path.exists() {
        download_file(url, &dl_path)?;
    } else {
        println!("  使用缓存: {}", dl_path.display());
    }

    // 目标目录
    let target_dir = paths::apps_dir().join(format!("{}-{}", name, ver));
    if target_dir.is_dir() {
        fs::remove_dir_all(&target_dir)
            .with_context(|| format!("无法清理旧版目录: {}", target_dir.display()))?;
    }
    fs::create_dir_all(&target_dir)?;

    // 处理下载文件
    match dl_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase().as_str() {
        "zip" => {
            super::extract_zip_to(&dl_path, &target_dir)?;
        }
        "7z" => {
            // 简单解压 7z
            let status = std::process::Command::new("7z")
                .args(["x", &dl_path.to_string_lossy(), &format!("-o{}", target_dir.to_string_lossy()), "-y"])
                .status()
                .ok();
            if status.map(|s| s.success()).unwrap_or(false) {
                println!("  已解压 7z 到: {}", target_dir.display());
            } else {
                anyhow::bail!("解压 7z 失败，请安装 7-Zip");
            }
        }
        _ => {
            // 单文件 → 直接复制
            let dest = target_dir.join(dl_path.file_name().unwrap());
            fs::copy(&dl_path, &dest)?;
        }
    }

    // 查找主 exe 创建快捷桩
    if let Some(entry_exe) = find_entry_exe(name, &target_dir) {
        create_shim(name, &entry_exe);
    }

    let path_str = target_dir.to_string_lossy().to_string();
    println!("  便携版已安装到: {}", path_str);
    record_installation(name, ver, "portable", &path_str);
    Ok(())
}

/// 查找软件目录中的主 exe
fn find_entry_exe(name: &str, dir: &Path) -> Option<std::path::PathBuf> {
    // 优先查找与软件名匹配的 exe
    let name_exe = dir.join(format!("{}.exe", name));
    if name_exe.exists() {
        return Some(name_exe);
    }

    // 查找第一个 exe
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("exe") {
                return Some(path);
            }
        }
    }
    None
}
