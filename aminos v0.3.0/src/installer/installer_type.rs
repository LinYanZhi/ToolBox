use std::fs;

use crate::installer::{download_file, get_download_path, run_silent, record_installation};

/// 安装 installer 类型软件
pub fn install_installer(name: &str, ver: &str, urls: &[String]) -> anyhow::Result<()> {
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

    match dl_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase().as_str() {
        "msi" => {
            run_silent(&dl_path, &[])?;
        }
        _ => {
            // exe: 常用静默参数
            let args = if name == "7zip" || name == "7z" {
                vec!["/S"]
            } else {
                vec!["/S", "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"]
            };
            run_silent(&dl_path, &args)?;
        }
    }

    println!("  {} 安装完成", name);
    record_installation(name, ver, "installer", "");
    Ok(())
}
