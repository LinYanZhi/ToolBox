use crate::software;

/// as freeze — 导出已安装清单
pub fn run() -> anyhow::Result<()> {
    let installed = software::read_installed()?;

    if installed.is_empty() {
        println!("  暂无已安装的软件");
        return Ok(());
    }

    println!();
    println!("  # 已安装软件清单");
    println!("  # 使用: as install <名称> 重新安装");
    println!();
    for (name, rec) in &installed {
        println!("  {}={}", name, rec.version);
    }
    println!();

    Ok(())
}
