use crate::software;
use crate::installer;

/// as uninstall — 卸载软件
pub fn run(name: String) -> anyhow::Result<()> {
    let installed = software::read_installed()?;

    if let Some(rec) = installed.get(&name) {
        println!("  卸载 {}...", name);
        installer::uninstall_software(&name, rec)?;
        software::remove_installed(&name)?;
        println!("  {} {} 已卸载", color::bold_green("完成"), name);
    } else {
        // 尝试从源中找到软件，用注册表卸载
        if let Some((matched, _entry)) = software::resolve_software(&name) {
            println!("  尝试卸载 {}（通过注册表）...", matched);
            installer::uninstall_via_registry(&matched)?;
        } else {
            eprintln!("  {} 未找到已安装的 '{}'", color::yellow("提示:"), name);
        }
    }

    Ok(())
}
