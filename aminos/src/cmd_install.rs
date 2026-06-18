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
                    Err(_) => {
                        // 3. 无源 → 搜索注册表
                        let reg_all = crate::registry::scan_all_installed_unfiltered();
                        let name_lower = n.to_lowercase();
                        let matches: Vec<_> = reg_all.into_iter()
                            .filter(|entry| {
                                entry.get("display_name")
                                    .map(|dn| dn.to_lowercase().contains(&name_lower))
                                    .unwrap_or(false)
                            })
                            .collect();
                        if !matches.is_empty() {
                            for reg in &matches {
                                let dn = reg.get("display_name").map(|s| s.as_str()).unwrap_or(&n);
                                let ver = reg.get("version").map(|s| s.as_str()).unwrap_or("未知");
                                let pub_ = reg.get("publisher").map(|s| s.as_str()).unwrap_or("");
                                eprintln!("  已在系统中找到: {}", dn);
                                eprintln!("    版本: {}", ver);
                                if !pub_.is_empty() {
                                    eprintln!("    发行商: {}", pub_);
                                }
                                eprintln!("    {} 没有对应的源定义，无法安装/更新。", color::yellow("注意:"));
                                eprintln!("    如需提交源定义，请前往: {}", crate::repo::SOURCE_GITHUB_URL);
                            }
                        } else {
                            eprintln!("  {} {}: 未找到源定义，且未在注册表中找到匹配", color::yellow("跳过"), name);
                        }
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
    let ver = opts.version.as_deref().unwrap_or(&sd.default_version);

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
    if let Err(e) = crate::installer::uninstall_software(name, false) {
        eprintln!("  {} 卸载旧版本失败: {}", color::yellow("警告"), e);
        eprintln!("  继续安装新版本...");
    } else {
        // 卸载后检查软件是否仍在系统中（用户可能取消了卸载）
        let vi = sd.versions.get(ver).unwrap();
        if let Some(ref detection) = vi.detection {
            if crate::registry::detect_installed(detection).is_some() {
                println!("  卸载未完成，已跳过安装");
                return Ok(());
            }
        }
    }

    // 安装新版本
    crate::installer::install_software_by_def(name, sd, opts)
}
