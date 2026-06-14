use std::fs;

use anyhow::Context;
use color;

use crate::{installer, paths, pe_version, software};

/// 更新 as 自身到最新版本。
pub fn run_self_update() -> anyhow::Result<()> {
    println!("正在检查 as 更新...");

    // 读取 as 自身的源定义
    let sd = software::read_software_def("as")?;
    let ver = &sd.default_version;
    let vi = sd.versions.get(ver)
        .ok_or_else(|| anyhow::anyhow!("as 版本 {} 未定义", ver))?;

    // 获取当前 as.exe 的 PE 版本，与源版本对比
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(current_ver) = pe_version::get_pe_version(&current_exe) {
            if current_ver == *ver {
                println!("  {} 已是最新版本。", color::green(ver));
                return Ok(());
            }
            println!("  当前版本: {}  →  最新版本: {}",
                color::yellow(&current_ver),
                color::cyan(ver),
            );
        } else {
            println!("  源版本: {}", color::cyan(ver));
        }
    }

    // 下载新版 as.zip
    let dl = paths::downloads_dir();
    fs::create_dir_all(&dl)?;

    let zip_name = format!("as-{}.zip", ver);
    let zip_path = dl.join(&zip_name);
    net::download::download_with_url_fallback("as", &vi.urls, &zip_path, &net::DownloadConfig::default())?;

    // 解压到临时目录
    let temp_dir = dl.join(format!("as-update-{}", ver));
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    installer::extract_zip_to(&zip_path, &temp_dir)?;

    // 找到 as.exe
    let new_exe = temp_dir.join("as.exe");
    if !new_exe.is_file() {
        anyhow::bail!("更新包中未找到 as.exe");
    }

    // 获取当前 as.exe 路径
    let current_exe = std::env::current_exe()
        .context("无法获取当前程序路径")?;

    println!("  正在更新: {}", current_exe.display());

    // 通过 PowerShell 脚本热替换（Windows 下不能直接覆盖正在运行的程序）
    let ps_script = format!(
        r#"
Start-Sleep -Seconds 1
Copy-Item -Path '{}' -Destination '{}' -Force
Remove-Item -Path '{}' -Recurse -Force
Write-Host '✓ as 已更新到 {}'
"#,
        new_exe.display(),
        current_exe.display(),
        temp_dir.display(),
        ver,
    );

    let ps_path = dl.join("update-as.ps1");
    fs::write(&ps_path, ps_script)?;

    // 启动 PowerShell 脚本，然后退出当前进程
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-ExecutionPolicy", "Bypass", "-File", &ps_path.to_string_lossy()])
        .spawn()?;

    println!("  ✓ 更新脚本已启动，as 将在重启后完成更新");
    println!("  当前终端可继续使用");

    Ok(())
}
