use crate::opts::InstallOpts;
use color;

pub fn run_install(opts: InstallOpts) -> anyhow::Result<()> {
    for name in &opts.names {
        let n = name.to_lowercase();

        // 1. 尝试第三方软件源
        match crate::software::read_software_def(&n) {
            Ok(sd) => {
                // 如果是自研工具（kind="self"），提示用 as tool install
                if sd.kind == "self" {
                    eprintln!("  {} {} 是自研工具，请改用:", color::yellow("提示"), name);
                    eprintln!("    {}", color::cyan(&format!("as tool install {}", name)));
                    continue;
                }
                // 正常安装第三方软件
                if let Err(e) = crate::installer::install_software_by_def(&n, &sd, &opts) {
                    eprintln!("  {} {}: {}", color::yellow("跳过"), name, e);
                }
            }
            Err(_) => {
                // 2. 不在第三方源中 → 尝试工具源
                match crate::software::read_tool_def(&n) {
                    Ok(_) => {
                        eprintln!("  {} {} 是自研工具，请使用:", color::yellow("提示"), name);
                        eprintln!("    {}", color::cyan(&format!("as tool install {}", name)));
                    }
                    Err(e) => {
                        eprintln!("  {} {}: {}", color::yellow("跳过"), name, e);
                    }
                }
            }
        }
    }
    Ok(())
}
