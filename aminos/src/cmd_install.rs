use crate::opts::InstallOpts;
use crate::cmd_names;
use color;

pub fn run_install(opts: InstallOpts) -> anyhow::Result<()> {
    for name in &opts.names {
        let n = name.to_lowercase();

        // 1. 尝试第三方软件源
        match crate::software::read_software_def(&n) {
            Ok(sd) => {
                // 如果是自研工具（kind="self"），提示用 as tool add
                if sd.kind == "self" {
                    eprintln!("  {} {} 是自研工具，请改用:", color::yellow("提示"), name);
                    eprintln!("    {}", color::cyan(&format!("{} {}", cmd_names::TOOL_ADD, name)));
                    continue;
                }

                // --upgrade 模式：先检测版本，卸载旧版后安装新版
                if opts.upgrade {
                    if let Err(e) = upgrade_and_install(&n, &sd, &opts) {
                        eprintln!("  {} {}: {}", color::yellow("跳过"), name, e);
                    }
                } else {
                    // 正常安装第三方软件
                    if let Err(e) = crate::installer::install_software_by_def(&n, &sd, &opts) {
                        eprintln!("  {} {}: {}", color::yellow("跳过"), name, e);
                    }
                }
            }
            Err(_) => {
                // 2. 不在第三方源中 → 尝试工具源
                match crate::software::read_tool_def(&n) {
                    Ok(_) => {
                        eprintln!("  {} {} 是自研工具，请使用:", color::yellow("提示"), name);
                        eprintln!("    {}", color::cyan(&format!("{} {}", cmd_names::TOOL_ADD, name)));
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

/// 升级模式：检测是否已安装，卸载旧版后安装新版
fn upgrade_and_install(name: &str, sd: &crate::software::SoftwareDef, opts: &InstallOpts) -> anyhow::Result<()> {
    let display = if sd.display_name.is_empty() { name } else { &sd.display_name };
    let ver = if opts.version.is_empty() { &sd.default_version } else { &opts.version };

    // 检查是否已安装
    let installed = {
        let vi = match sd.versions.get(ver) {
            Some(vi) => vi,
            None => {
                eprintln!("  {} 版本 {} 未找到定义", display, ver);
                return Ok(());
            }
        };

        if let Some(ref detection) = vi.detection {
            crate::registry::detect_installed(detection).is_some()
        } else {
            false
        }
    };

    if !installed {
        // 未安装，直接安装
        return crate::installer::install_software_by_def(name, sd, opts);
    }

    println!("  {} 检测到旧版本，正在卸载...", display);

    // 卸载旧版本
    if let Err(e) = crate::installer::uninstall_software(name, false, false) {
        eprintln!("  {} 卸载旧版本失败: {}", color::yellow("警告"), e);
        eprintln!("  继续安装新版本...");
    }

    // 安装新版本
    crate::installer::install_software_by_def(name, sd, opts)
}
