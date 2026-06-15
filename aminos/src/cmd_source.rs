use color;

use crate::{paths, software};

pub fn run_source(action: crate::SourceCmd) -> anyhow::Result<()> {
    match action {
        crate::SourceCmd::Update => {
            software::update_sources()?;
        }
        crate::SourceCmd::Path { open } => {
            let dir = paths::source_dir();
            if open {
                let _ = std::process::Command::new("explorer").arg(&dir).spawn();
                println!("已在资源管理器中打开: {}", dir.display());
            } else {
                println!("{}", dir.display());
            }
        }
    }
    Ok(())
}

pub fn run_dirs(open_explorer: bool) -> anyhow::Result<()> {
    let root = std::env::var("LOCALAPPDATA")
        .map(|p| std::path::PathBuf::from(p).join("aminos"))
        .unwrap_or_else(|_| paths::source_dir().parent().map(|p| p.to_path_buf()).unwrap_or_default());

    if open_explorer {
        let _ = std::process::Command::new("explorer").arg(&root).spawn();
        println!("已在资源管理器中打开: {}", root.display());
        return Ok(());
    }

    let exe = std::env::current_exe().unwrap_or_default();
    println!("\n{}\n", color::bold_cyan("aminos 数据目录一览"));

    println!("  {}", color::bold_yellow("可执行文件"));
    println!("    {}", exe.display());

    println!();
    println!("  {}  (json)", color::bold_yellow("软件源定义"));
    println!("    {}", paths::source_dir().display());

    println!();
    println!("  {}  (下载的 exe/msi/zip)", color::bold_yellow("安装包缓存"));
    println!("    {}", paths::downloads_dir().display());

    println!();
    println!("  {}  (installed.json)", color::bold_yellow("安装记录"));
    println!("    {}", paths::installed_json().display());

    println!();
    println!("  {}  (as 安装的软件链接)", color::bold_yellow("快捷方式"));
    println!("    {}", paths::apps_dir().display());

    println!();
    println!("  {}", color::bold_yellow("数据根目录"));
    println!("    {}", root.display());

    Ok(())
}
