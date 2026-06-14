use color::{self, DisplayWidth, pad_left as pad};

use crate::software;

pub fn run_tool(action: crate::ToolCmd) -> anyhow::Result<()> {
    match action {
        crate::ToolCmd::List => {
            let defs = software::list_software_defs().unwrap_or_default();
            let db = software::read_installed_db().unwrap_or_default();

            // 筛选所有自研工具（kind=self）
            let self_tools: Vec<&software::SoftwareDef> = defs.iter()
                .filter(|d| d.kind == "self")
                .collect();

            if self_tools.is_empty() {
                println!("没有可用的自研工具。");
                return Ok(());
            }

            println!("{}", color::bold_cyan("可用自研工具:"));
            println!();

            let max_name_w = self_tools.iter()
                .map(|d| d.display_name.as_str())
                .max_by_key(|n| {
                    use color::DisplayWidth;
                    n.display_width()
                })
                .map(|n| {
                    use color::DisplayWidth;
                    n.display_width()
                })
                .unwrap_or(10)
                .max(10);

            let max_ver_w = self_tools.iter()
                .map(|d| d.default_version.display_width())
                .max()
                .unwrap_or(10);

            for def in &self_tools {
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
        }
        crate::ToolCmd::Remove { name } => {
            crate::installer::uninstall_software(&name, false, false)?;
        }
    }
    Ok(())
}
