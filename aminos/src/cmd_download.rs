use std::path::PathBuf;

use color::*;
use crate::opts::DownloadOpts;
use crate::paths;

pub fn run_download(opts: DownloadOpts) -> anyhow::Result<()> {
    // -o/--open：打开下载目录
    if opts.open {
        let dir = paths::downloads_dir();
        if dir.exists() {
            let _ = std::process::Command::new("explorer").arg(&dir).spawn();
            println!("已在资源管理器中打开: {}", gray(dir.to_string_lossy()));
        } else {
            println!("下载目录不存在: {}", gray(dir.to_string_lossy()));
        }
        return Ok(());
    }

    if opts.targets.is_empty() {
        anyhow::bail!("请指定要下载的软件名称或链接");
    }

    // 确定下载目标目录
    let target_dir = if let Some(ref custom) = opts.target_dir {
        let dir = PathBuf::from(custom);
        std::fs::create_dir_all(&dir)?;
        dir
    } else {
        let dir = paths::downloads_dir();
        std::fs::create_dir_all(&dir)?;
        dir
    };

    let count = opts.targets.len();
    for (i, target) in opts.targets.iter().enumerate() {
        // 判断是 URL 还是软件名称
        if target.starts_with("http://") || target.starts_with("https://") || target.starts_with("ftp://") {
            // 直接下载 URL
            download_url(target, &target_dir)?;
        } else {
            // 尝试匹配软件名称
            download_by_name(target, &target_dir)?;
        }
        if i < count - 1 {
            println!();
        }
    }

    Ok(())
}

/// 通过 URL 下载文件到指定目录
fn download_url(url: &str, target_dir: &std::path::Path) -> anyhow::Result<()> {
    // 从 URL 提取文件名
    let filename = url.split('/')
        .filter(|s| !s.is_empty())
        .last()
        .unwrap_or("download");

    let dest = target_dir.join(filename);
    println!("{} {}", bold_green("正在下载"), gray(url));
    println!("  {}", gray(dest.to_string_lossy()));

    net::download::download_with_url_fallback(
        "download",
        &[url.to_string()],
        &dest,
        &net::DownloadConfig::default(),
    )?;

    println!("{} {}", green("下载完成"), gray(dest.to_string_lossy()));
    Ok(())
}

/// 通过软件名称下载
fn download_by_name(name: &str, target_dir: &std::path::Path) -> anyhow::Result<()> {
    let n = name.to_lowercase();

    // 先尝试第三方软件源
    if let Ok(sd) = crate::software::read_software_def(&n) {
        let ver = &sd.default_version;
        let vi = match sd.versions.get(ver) {
            Some(vi) => vi,
            None => anyhow::bail!("{}: 版本 {} 未定义下载地址", name, ver),
        };

        if vi.urls.is_empty() {
            anyhow::bail!("{}: 未配置下载地址", name);
        }

        let display = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };
        let ext = vi.urls[0].split('.').last().unwrap_or("zip");
        let zip_name = format!("{}-{}.{}", sd.name, ver, ext);
        let dest = target_dir.join(&zip_name);

        println!("{} {} {}...", bold_green("正在下载"), bold_cyan(display), gray(ver));
        for url in &vi.urls {
            println!("  {}", gray(url));
        }
        net::download::download_with_url_fallback(&sd.name, &vi.urls, &dest, &net::DownloadConfig::default())?;
        println!("{} {}", green("下载完成"), gray(dest.to_string_lossy()));
        return Ok(());
    }

    // 再尝试工具源
    if let Ok(sd) = crate::software::read_tool_def(&n) {
        let ver = &sd.default_version;
        let vi = match sd.versions.get(ver) {
            Some(vi) => vi,
            None => anyhow::bail!("{}: 版本 {} 未定义下载地址", name, ver),
        };

        if vi.urls.is_empty() {
            anyhow::bail!("{}: 未配置下载地址", name);
        }

        let display = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };
        let zip_name = format!("{}-{}.zip", sd.name, ver);
        let dest = target_dir.join(&zip_name);

        println!("{} {} {}...", bold_green("正在下载"), bold_cyan(display), gray(ver));
        for url in &vi.urls {
            println!("  {}", gray(url));
        }
        net::download::download_with_url_fallback(&sd.name, &vi.urls, &dest, &net::DownloadConfig::default())?;
        println!("{} {}", green("下载完成"), gray(dest.to_string_lossy()));
        return Ok(());
    }

    anyhow::bail!("未找到软件或工具 '{}'，请检查名称是否正确", name)
}
