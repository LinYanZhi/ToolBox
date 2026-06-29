use color::*;
use crate::installer;
use crate::software;

pub fn run(name: &str) -> anyhow::Result<()> {
    let (matched_name, entry) = match software::resolve(name) {
        Some(r) => r,
        None => {
            eprintln!("  {} 未找到软件 '{}'", yellow("错误"), bold_cyan(name));
            return Ok(());
        }
    };

    if let Some(detect) = &entry.detect {
        println!();
        println!("  卸载 {} (安装版)", bold_cyan(&matched_name));
        println!();

        installer::uninstall_installer(&matched_name, Some(detect))?;
    } else {
        eprintln!("  {} '{}' 没有注册表检测配置，无法自动卸载", yellow("提示"), bold_cyan(&matched_name));
        eprintln!("    如果是便携版，请手动删除下载的文件");
    }

    println!();

    Ok(())
}