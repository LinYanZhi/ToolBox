use color::*;

pub fn run() -> anyhow::Result<()> {
    println!();
    println!("  {} 检查更新...", bold_cyan("更新"));
    println!();

    println!("  {} 当前版本: {}", bold_green("as"), env!("CARGO_PKG_VERSION"));
    println!("  {} 暂未实现自动更新，请手动下载最新版本", gray("提示"));
    println!();

    Ok(())
}