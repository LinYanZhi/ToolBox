use color::{self, DisplayWidth, pad_left as pad};

use crate::opts::ToolAddOpts;
use crate::{installer, software, cmd_names};

/// 添加（安装/升级）自研工具
pub fn run_add(opts: ToolAddOpts) -> anyhow::Result<()> {
    let mut failed = 0u32;

    for name in &opts.names {
        let n = name.to_lowercase();

        if opts.upgrade {
            // 升级模式：检查版本，需要时更新
            let sd = match software::read_tool_def(&n) {
                Ok(sd) => sd,
                Err(e) => {
                    eprintln!("  {} {}: {}", color::yellow("跳过"), name, e);
                    failed += 1;
                    continue;
                }
            };

            let display = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };
            let source_ver = &sd.default_version;
            let installed_db = software::read_installed_db().unwrap_or_default();
            let current_ver = installed_db.get(&n)
                .map(|rec| rec.version.clone())
                .unwrap_or_default();
            let recorded_sha256 = installed_db.get(&n)
                .map(|rec| rec.file_sha256.clone())
                .unwrap_or_default();
            let source_vi = sd.versions.get(source_ver);
            let source_sha256 = source_vi.map(|vi| vi.sha256.as_str()).unwrap_or("");
            let sha256_changed = !source_sha256.is_empty() && source_sha256 != recorded_sha256;

            if current_ver == *source_ver && !sha256_changed && !opts.renew {
                println!("  {}", color::gray(format!("{} {} 已是最新", display, current_ver)));
                continue;
            }

            let reason = if sha256_changed {
                format!("内容已变更 (SHA256)")
            } else {
                format!("{} → {}", current_ver, source_ver)
            };
            println!("  ▶ {} {} ...", display, reason);
        }

        let install_opts = crate::opts::InstallOpts {
            names: vec![n.clone()],
            version: opts.version.clone(),
            gui: false,
            renew: opts.renew,
            download_only: opts.download_only,
            inst_type: None,
            upgrade: false,
        };
        if let Err(e) = installer::install_tool(&n, &install_opts) {
            eprintln!("  {} {}: {}", color::yellow("跳过"), name, e);
            failed += 1;
        }
    }

    if failed > 0 {
        eprintln!("  {} 个操作失败", failed);
    }
    Ok(())
}

/// 列出已安装的自研工具
pub fn run_list() -> anyhow::Result<()> {
    let defs = software::list_tool_defs().unwrap_or_default();
    let db = software::read_installed_db().unwrap_or_default();

    if defs.is_empty() {
        println!("没有可用的自研工具。");
        println!("{}", color::gray(format!("请先运行: {}", cmd_names::SOURCE_UPDATE_HINT)));
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
    println!("{}", color::gray(format!("安装: {} <名称>   卸载: {} <名称>", cmd_names::TOOL_ADD, cmd_names::TOOL_REMOVE)));
    Ok(())
}

/// 移除自研工具
pub fn run_remove(name: &str) -> anyhow::Result<()> {
    let n = name.to_lowercase();
    installer::uninstall_tool(&n)
}
