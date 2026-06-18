use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::bail;

use sha2::{Sha256, Digest};

use crate::paths;

/// 获取或下载安装包。
///
/// 先检查缓存目录，匹配已下载的文件。未命中则下载并用魔数修正扩展名。
/// 返回下载后的完整路径。
pub(crate) fn get_installer_path(name: &str, version: &str, urls: &[String], renew: bool) -> anyhow::Result<PathBuf> {
    let dl = paths::downloads_dir();
    fs::create_dir_all(&dl)?;

    // 1) 从 URL 探测文件名（HEAD 或路径末段）
    let filename = safe_installer_name(name, version, urls);
    let needs_magic_fix = urls.first().map_or(true, |u| {
        !u.rsplit('.').next().map_or(false, |ext| {
            matches!(ext.to_lowercase().as_str(), "exe" | "msi" | "zip" | "7z" | "rar" | "tar" | "gz" | "xz" | "bz2" | "appx")
        })
    });

    // 2) 检查缓存
    let target = dl.join(&filename);
    if target.is_file() && !renew {
        println!("  使用缓存文件: {}", target.display());
        return Ok(target);
    }

    // 前缀匹配：扫描 {name}-{version}.xxxxx（应对魔数修正或 probe 返回不同文件名）
    if !renew {
        let base = format!("{}-{}.",
            name.to_lowercase().replace(' ', "-"),
            version.to_lowercase().replace(' ', "-"));
        if let Ok(entries) = std::fs::read_dir(&dl) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.starts_with(&base) && fname != filename {
                    let p = entry.path();
                    if p.is_file() {
                        println!("  使用缓存文件: {}", p.display());
                        return Ok(p);
                    }
                }
            }
        }
    }

    // 3) 下载到临时文件
    let tmp = dl.join(format!("{}.downloading", filename));
    net::download::download_with_url_fallback(name, urls, &tmp, &net::DownloadConfig::default().renew(renew))?;

    // 4) 魔数修正扩展名（仅当 URL 探测失败、用猜测文件名时）
    let corrected = if needs_magic_fix {
        match net::detect_format(&tmp) {
            Some(fmt) => {
                let ext = fmt.extension();
                if !filename.ends_with(ext) {
                    let corrected = format!("{}-{}{}",
                        name.to_lowercase().replace(' ', "-"),
                        version.to_lowercase().replace(' ', "-"), ext);
                    let p = dl.join(&corrected);
                    fs::rename(&tmp, &p)?;
                    p
                } else {
                    let p = dl.join(&filename);
                    fs::rename(&tmp, &p)?;
                    p
                }
            }
            None => {
                let p = dl.join(&filename);
                fs::rename(&tmp, &p)?;
                p
            }
        }
    } else {
        let p = dl.join(&filename);
        fs::rename(&tmp, &p)?;
        p
    };

    // 5) 最终验证
    if !net::verify_downloaded_file(&corrected) {
        let _ = std::fs::remove_file(&corrected);
        bail!("{}: 下载后验证失败（文件损坏或反盗链页面）", name);
    }

    Ok(corrected)
}

/// 构造安全的安装包文件名。
pub(crate) fn safe_installer_name(name: &str, version: &str, urls: &[String]) -> String {
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

/// 计算文件的 SHA256 十六进制字符串。
pub(crate) fn file_sha256(path: &Path) -> String {
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => return String::new(),
        };
        hasher.update(&buf[..n]);
    }
    hex::encode(hasher.finalize())
}
