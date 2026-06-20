use crate::cmd_names;
use crate::opts::UninstallOpts;
use color;

/// 无参数时显示自定义用法
pub fn print_usage() {
    println!();
    println!("  {}  {}", color::bold_cyan("uninstall"), color::bright_black("卸载指定软件"));
    println!();
    println!("  {} {}", color::bright_black("用法:"), color::bold(&format!("{} [选项] <软件名称...>", cmd_names::UNINSTALL)));
    println!();
    println!("  {}", color::bright_black("选项:"));
    println!("    {} {}  {}", color::cyan("-f"), color::cyan("--force"), color::bright_black("强制删除（跳过卸载器）"));
    println!("    {} {}  {}", color::cyan("-h"), color::cyan("--help"), color::bright_black("显示帮助"));
    println!();
    println!("  {}", color::bright_black("示例:"));
    println!("    {}  {}", color::bold(&format!("{} 7zip", cmd_names::UNINSTALL)), color::bright_black("弹出卸载窗口卸载 7-Zip"));
    println!("    {}  {}", color::bold(&format!("{} 7zip --force", cmd_names::UNINSTALL)), color::bright_black("强制删除（跳过卸载器）"));
    println!();
}

pub fn run_uninstall(opts: UninstallOpts) -> anyhow::Result<()> {
    for name in &opts.names {
        let n = name.to_lowercase();
        if let Err(e) = crate::installer::uninstall_software(&n, opts.force) {
            eprintln!("  {} {}: {}", color::yellow("跳过"), name, e);
        }
    }
    Ok(())
}
