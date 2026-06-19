use crate::opts::UninstallOpts;
use color;

/// 无参数时显示自定义用法
pub fn print_usage() {
    println!();
    println!("  {}  {}", color::bold_cyan("uninstall"), color::gray("卸载指定软件"));
    println!();
    println!("  {} {}", color::gray("用法:"), color::bold("as uninstall [选项] <软件名称...>"));
    println!();
    println!("  {}", color::gray("选项:"));
    println!("    -f, --force  强制删除（跳过卸载器）");
    println!("    -h, --help   显示帮助");
    println!();
    println!("  {}", color::gray("示例:"));
    println!("    {}  {}", color::bold("as uninstall 7zip"), color::gray("弹出卸载窗口卸载 7-Zip"));
    println!("    {}  {}", color::bold("as uninstall 7zip --force"), color::gray("强制删除（跳过卸载器）"));
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
