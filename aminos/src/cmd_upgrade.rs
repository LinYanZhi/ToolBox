use crate::opts::{InstallOpts, UpgradeOpts};
use crate::{paths, registry, software};
use color;

pub fn run_upgrade(opts: UpgradeOpts) -> anyhow::Result<()> {
    let installed_db = software::read_installed_db().unwrap_or_default();

    let targets: Vec<String> = if opts.names.is_empty() {
        installed_db.keys().cloned().collect()
    } else {
        opts.names.iter().map(|n| n.to_lowercase()).collect()
    };

    if targets.is_empty() {
        println!("没有已安装的软件需要升级。");
        return Ok(());
    }

    let mut updated = 0u32;
    let mut up_to_date = 0u32;
    let mut failed = 0u32;

    for name in &targets {
        let sd = match software::read_software_def(name) {
            Ok(sd) => sd,
            Err(e) => {
                eprintln!("  {} {}: {}", color::yellow("跳过"), name, e);
                failed += 1;
                continue;
            }
        };

        let display = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };
        let source_ver = &sd.default_version;

        // 判断是否为便携版
        let is_portable = sd.versions.get(source_ver)
            .map(|vi| vi.installer_type == "portable")
            .unwrap_or(false);

        // 便携版：检查 apps/{name}-{version} 目录是否存在
        // 标准版：从注册表获取版本
        let portable_dir = paths::apps_dir().join(format!("{}-{}", name, source_ver));
        let current_ver: String = if is_portable {
            if portable_dir.is_dir() {
                installed_db.get(name)
                    .map(|rec| rec.version.clone())
                    .unwrap_or_else(|| source_ver.to_string())
            } else {
                // 目录已被手动删除
                String::new()
            }
        } else {
            // 标准安装：从注册表获取版本
            let registry_ver = sd.versions.get(source_ver)
                .and_then(|vi| vi.detection.as_ref())
                .and_then(|d| registry::detect_installed(d))
                .and_then(|r| r.get("DisplayVersion").cloned());
            installed_db.get(name)
                .map(|rec| rec.version.clone())
                .or(registry_ver)
                .unwrap_or_default()
        };

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
        // 从安装记录中读取之前的安装类型，升级时复用
        let prev_inst_type = installed_db.get(name)
            .and_then(|rec| {
                if rec.installer_type.is_empty() { None } else { Some(rec.installer_type.clone()) }
            });
        let upgrade_opts = InstallOpts::new(vec![name.to_string()], None, false, opts.renew, false, prev_inst_type);
        match crate::installer::install_software(name, &upgrade_opts) {
            Ok(()) => {
                updated += 1;
            }
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
