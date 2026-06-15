use color::{self, DisplayWidth, pad_left as pad};

use crate::opts::{ToolInstallOpts, ToolUpgradeOpts};
use crate::{installer, software};

/// 安装自研工具
pub fn run_install(opts: ToolInstallOpts) -> anyhow::Result<()> {
    let mut failed = 0u32;
    for name in &opts.names {
        let n = name.to_lowercase();
        let install_opts = crate::opts::InstallOpts::new(
            vec![n.clone()],
            if opts.version.is_empty() { None } else { Some(opts.version.clone()) },
            false,
            opts.renew,
            opts.download_only,
            None,
        );
        if let Err(e) = installer::install_tool(&n, &install_opts) {
            eprintln!("  {} {}: {}", color::yellow("跳过"), name, e);
            failed += 1;
        }
    }
    if failed > 0 {
        eprintln!("  {} 个安装失败", failed);
    }
    Ok(())
}

/// 升级自研工具
pub fn run_upgrade(opts: ToolUpgradeOpts) -> anyhow::Result<()> {
    let installed_db = software::read_installed_db().unwrap_or_default();
    let defs = software::list_tool_defs().unwrap_or_default();

    let targets: Vec<String> = if opts.names.is_empty() {
        // 从 installed db 中查找所有自研工具
        defs.iter()
            .filter(|d| installed_db.contains_key(&d.name))
            .map(|d| d.name.clone())
            .collect()
    } else {
        opts.names.iter().map(|n| n.to_lowercase()).collect()
    };

    if targets.is_empty() {
        println!("没有已安装的自研工具需要升级。");
        return Ok(());
    }

    let mut updated = 0u32;
    let mut up_to_date = 0u32;
    let mut failed = 0u32;

    for name in &targets {
        let sd = match software::read_tool_def(name) {
            Ok(sd) => sd,
            Err(e) => {
                eprintln!("  {} {}: {}", color::yellow("跳过"), name, e);
                failed += 1;
                continue;
            }
        };

        let display = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };
        let source_ver = &sd.default_version;

        let current_ver = installed_db.get(name)
            .map(|rec| rec.version.clone())
            .unwrap_or_default();

        if current_ver == *source_ver && !opts.renew {
            println!("  {}", color::gray(format!("{} {} 已是最新", display, current_ver)));
            up_to_date += 1;
            continue;
        }

        if opts.check {
            println!("  {} → {} 可更新",
                color::yellow(format!("{} {}", display, current_ver)),
                color::green(source_ver));
            updated += 1;
            continue;
        }

        println!("  ▶ {} {} → {} ...", display, current_ver, source_ver);
        let install_opts = crate::opts::InstallOpts::new(
            vec![name.clone()], None, false, opts.renew, false, None,
        );
        match installer::install_tool(name, &install_opts) {
            Ok(()) => updated += 1,
            Err(e) => {
                eprintln!("  {}: {}", color::yellow(format!("升级 {} 失败", display)), e);
                failed += 1;
            }
        }
    }

    println!();
    if opts.check {
        println!("{}",
            color::gray(format!("共检查 {} 个，{} 个可更新，{} 个最新，{} 个失败",
                targets.len(), updated, up_to_date, failed)));
    } else {
        println!("{}",
            color::gray(format!("共 {} 个，{} 个已升级，{} 个已最新，{} 个失败",
                targets.len(), updated, up_to_date, failed)));
    }
    Ok(())
}

/// 列出自研工具
pub fn run_list() -> anyhow::Result<()> {
    let defs = software::list_tool_defs().unwrap_or_default();
    let db = software::read_installed_db().unwrap_or_default();

    if defs.is_empty() {
        println!("没有可用的自研工具。");
        println!("{}", color::gray("请先运行: as env source update"));
        return Ok(());
    }

    println!("{}", color::bold_cyan("可用自研工具:"));
    println!();

    let max_name_w = defs.iter()
        .map(|d| {
            let name = if d.display_name.is_empty() { &d.name } else { &d.display_name };
            name.display_width()
        })
        .max()
        .unwrap_or(10)
        .max(10);

    let max_ver_w = defs.iter()
        .map(|d| d.default_version.display_width())
        .max()
        .unwrap_or(10);

    for def in &defs {
        let name = if def.display_name.is_empty() { &def.name } else { &def.display_name };
        let ver = &def.default_version;
        let installed = db.contains_key(&def.name);
        let status = if installed {
            color::green(format!("{}  {}",
                color::bold_green("已安装"),
                color::cyan(&pad(ver, max_ver_w))))
        } else {
            color::gray(format!("{}  {}",
                "未安装",
                &pad("", max_ver_w)))
        };
        println!("  {}  {}  {}", color::cyan(pad(name, max_name_w)), status, color::gray(&def.description));
    }

    println!();
    println!("{}", color::gray("安装: as tool install <名称>   卸载: as tool remove <名称>"));
    Ok(())
}

/// 移除自研工具
pub fn run_remove(name: &str) -> anyhow::Result<()> {
    let n = name.to_lowercase();
    installer::uninstall_tool(&n)
}
