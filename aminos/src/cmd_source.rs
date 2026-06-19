use crate::{paths, software};

pub fn run_source(cmd: &crate::opts::SourceCommand) -> anyhow::Result<()> {
    match cmd {
        crate::opts::SourceCommand::Update => run_update(),
        crate::opts::SourceCommand::Clear => run_clear(),
        crate::opts::SourceCommand::Open => run_open(),
        crate::opts::SourceCommand::Speedtest { name, software } => {
            let names = name.as_ref().map(|n| vec![n.clone()]).unwrap_or_default();
            crate::speedtest::speedtest(&names, *software)
        }
        crate::opts::SourceCommand::Add { name, url } => run_add(name, url),
        crate::opts::SourceCommand::Remove { name } => run_remove(name),
        crate::opts::SourceCommand::List => run_list(),
        crate::opts::SourceCommand::Tree => run_tree(),
        crate::opts::SourceCommand::Info { name } => run_info(name),
        crate::opts::SourceCommand::Enable { name } => run_toggle(name, true),
        crate::opts::SourceCommand::Disable { name } => run_toggle(name, false),
    }
}

fn run_update() -> anyhow::Result<()> {
    software::update_sources()
}

fn run_clear() -> anyhow::Result<()> {
    let dir = paths::source_dir();
    if !dir.exists() {
        println!("源目录不存在: {}", dir.display());
        return Ok(());
    }

    let total = count_files(&dir);
    // 清空所有分类目录 + tools + community
    for sub in paths::CATEGORIES {
        let subdir = dir.join(sub);
        if subdir.exists() {
            let _ = std::fs::remove_dir_all(&subdir);
        }
        let _ = std::fs::create_dir_all(&subdir);
    }
    // tools
    let tools_dir = dir.join("tools");
    if tools_dir.exists() {
        let _ = std::fs::remove_dir_all(&tools_dir);
    }
    let _ = std::fs::create_dir_all(&tools_dir);
    // community
    let comm_dir = dir.join("community");
    if comm_dir.exists() {
        let _ = std::fs::remove_dir_all(&comm_dir);
    }
    println!("已清空源缓存（共 {} 个文件）", total);
    Ok(())
}

fn run_open() -> anyhow::Result<()> {
    let dir = paths::source_dir();
    if dir.exists() {
        let _ = std::process::Command::new("explorer").arg(&dir).spawn();
        println!("已在资源管理器中打开: {}", dir.display());
    } else {
        println!("源目录不存在: {}", dir.display());
    }
    Ok(())
}

fn run_add(name: &str, url: &str) -> anyhow::Result<()> {
    let config_dir = paths::config_dir();
    let cfg = config::SourceConfig::new(config_dir);
    cfg.add(name, url)?;
    println!("  正在首次同步源 '{}'...", name);
    let dest = paths::community_source_named(name);
    let repo = config::SourceRepo::new(vec![url.to_string()]);
    if let Err(e) = config::source::update_sources(&dest, &repo) {
        eprintln!("  {} 首次同步失败: {}（可稍后运行 `as source update` 重试）", color::red("警告"), e);
    } else {
        println!("  首次同步完成");
    }
    Ok(())
}

fn run_remove(name: &str) -> anyhow::Result<()> {
    let config_dir = paths::config_dir();
    let cfg = config::SourceConfig::new(config_dir);
    cfg.remove(name)?;
    Ok(())
}

fn run_list() -> anyhow::Result<()> {
    let source_root = paths::source_dir();
    let base_url = crate::repo::SOURCE_RAW_URL;

    println!();
    println!("  {}  ({} 更新)", color::bold_cyan("软件源列表"), color::gray("as source update"));
    println!("  {}", color::gray("─".repeat(56)));

    // 内置源（分类）
    let mut total_builtin = 0;
    for (dir_name, label, desc) in paths::CATEGORY_META {
        let dir = source_root.join(dir_name);
        let count = count_json_files(&dir);
        total_builtin += count;
        println!("  {} {}  {}  {} 个定义  {}",
            color::green("✓"),
            color::cyan(label),
            color::gray(desc),
            color::yellow(&count.to_string()),
            color::gray(&format!("({})", dir_name)),
        );
        println!("    {}", color::gray(&format!("{}/apps/{}", base_url, dir_name)));
    }

    // 自研工具
    let tools_dir = paths::tools_source_dir();
    let tools_count = count_json_files(&tools_dir);
    total_builtin += tools_count;
    println!("  {} {}  {}  {} 个定义  {}",
        color::green("✓"),
        color::cyan("自研工具"),
        color::gray("ToolBox 系列 CLI"),
        color::yellow(&tools_count.to_string()),
        color::gray("(tools)"),
    );
    println!("    {}", color::gray(&format!("{}/tools", base_url)));

    println!("  {} 共计 {} 个软件定义", color::gray("─"), total_builtin);

    // 社区源
    let config_dir = paths::config_dir();
    let cfg = config::SourceConfig::new(config_dir);
    let entries = cfg.load();

    if entries.is_empty() {
        println!();
        println!("  {}", color::gray("暂无第三方社区源"));
        println!("  使用 {} 添加社区源", color::yellow("as source add <名称> <URL>"));
    } else {
        println!();
        println!("  {}  ({} 管理)", color::bold_cyan("第三方社区源"), color::gray("as source add|remove|enable|disable"));
        for entry in &entries {
            let status_mark = if entry.enabled { color::green("●") } else { color::gray("○") };
            let status_label = if entry.enabled { color::green("已启用") } else { color::gray("已禁用") };
            let local_dir = paths::community_source_named(&entry.name);
            let count = count_json_files(&local_dir);
            let count_str = if count > 0 {
                format!("{} 个定义", count)
            } else {
                "尚未同步".to_string()
            };
            println!();
            println!("  {}  {}  ({})", status_mark, color::cyan(&entry.name), status_label);
            println!("    地址: {}", color::gray(&entry.url));
            println!("    本地: {}", color::gray(&count_str));
        }
    }

    println!();
    Ok(())
}

fn run_tree() -> anyhow::Result<()> {
    let source_root = paths::source_dir();
    let base_url = crate::repo::SOURCE_RAW_URL;

    println!();
    println!("  {}", color::bold_cyan("软件源"));
    println!("  {}", color::gray("─".repeat(56)));

    let meta_len = paths::CATEGORY_META.len();
    let all_items: Vec<(&str, &str, &str)> = paths::CATEGORY_META.iter().map(|(a, b, c)| (*a, *b, *c)).collect();

    for (i, (dir_name, label, desc)) in all_items.iter().enumerate() {
        let is_last_dir = i == meta_len - 1;
        let dir = source_root.join(dir_name);
        let count = count_json_files(&dir);

        let mut defs: Vec<software::SoftwareDef> = Vec::new();
        if dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for e in entries.flatten() {
                    let p = e.path();
                    if p.extension().map_or(false, |ext| ext == "json") && p.file_name().and_then(|n| n.to_str()) != Some("index.json") {
                        if let Ok(sd) = software::parse_json(&p) {
                            defs.push(sd);
                        }
                    }
                }
            }
        }
        defs.sort_by(|a, b| a.name.cmp(&b.name));

        let prefix = if is_last_dir { "└── " } else { "├── " };
        let connector = if is_last_dir { "    " } else { "│   " };
        println!("  {} {}",
            color::gray(prefix),
            color::cyan(label),
        );
        println!("  {} {}  {}",
            connector,
            color::gray(&format!("[{}] {}", dir_name, desc)),
            color::gray(&format!("({} 个)", count)),
        );

        for (j, sd) in defs.iter().enumerate() {
            let is_last_def = j == defs.len() - 1;
            let branch = if is_last_def { "└── " } else { "├── " };
            let dn = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };
            println!("  {}  {}  {}",
                color::gray(&format!("{}    {}", connector, branch)),
                software::paint_software(dn, sd),
                color::gray(&format!("({})", sd.name)),
            );
        }

        if defs.is_empty() && count > 0 {
            println!("  {}    {} ({} 个文件, 暂未同步)", connector, color::gray("└──"), count);
        }

        println!("  {}    {}",
            connector,
            color::gray(&format!("地址: {}/apps/{}", base_url, dir_name)),
        );
        println!("  {}", connector);
    }

    // 自研工具
    let tools_dir = paths::tools_source_dir();
    let tools_count = count_json_files(&tools_dir);
    let mut tool_defs: Vec<software::SoftwareDef> = Vec::new();
    if tools_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&tools_dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().map_or(false, |ext| ext == "json") && p.file_name().and_then(|n| n.to_str()) != Some("index.json") {
                    if let Ok(sd) = software::parse_json(&p) {
                        tool_defs.push(sd);
                    }
                }
            }
        }
    }
    tool_defs.sort_by(|a, b| a.name.cmp(&b.name));
    println!("  {} {}",
        color::gray("└── "),
        color::cyan("自研工具"),
    );
    println!("  {} {}",
        color::gray("    "),
        color::gray(&format!("(tools, {} 个)", tools_count)),
    );
    for (j, sd) in tool_defs.iter().enumerate() {
        let is_last = j == tool_defs.len() - 1;
        let branch = if is_last { "└── " } else { "├── " };
        let dn = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };
        if sd.display_name.is_empty() {
            println!("  {}  {}",
                color::gray(&format!("    {}    {}", "", branch)),
                software::paint_software(dn, sd),
            );
        } else {
            println!("  {}  {}  {}",
                color::gray(&format!("    {}    {}", "", branch)),
                software::paint_software(dn, sd),
                color::gray(&format!("({})", sd.name)),
            );
        }
    }
    println!("  {}    {}",
        color::gray("    "),
        color::gray(&format!("地址: {}/tools", base_url)),
    );
    println!("  {}", color::gray("    "));

    // 社区源
    let _ = run_tree_community();

    println!();
    Ok(())
}

fn run_tree_community() -> anyhow::Result<()> {
    let config_dir = paths::config_dir();
    let cfg = config::SourceConfig::new(config_dir);
    let entries = cfg.load();

    if entries.is_empty() {
        return Ok(());
    }

    println!();
    println!("  {}", color::bold_cyan("第三方社区源"));
    for (i, entry) in entries.iter().enumerate() {
        let is_last = i == entries.len() - 1;
        let prefix = if is_last { "└── " } else { "├── " };
        let connector = if is_last { "    " } else { "│   " };
        let status = if entry.enabled { "已启用" } else { "已禁用" };

        let local_dir = paths::community_source_named(&entry.name);
        let mut defs: Vec<software::SoftwareDef> = Vec::new();
        if local_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&local_dir) {
                for e in entries.flatten() {
                    let p = e.path();
                    if p.extension().map_or(false, |ext| ext == "json") && p.file_name().and_then(|n| n.to_str()) != Some("index.json") {
                        if let Ok(sd) = software::parse_json(&p) {
                            defs.push(sd);
                        }
                    }
                }
            }
        }
        defs.sort_by(|a, b| a.name.cmp(&b.name));

        println!("  {} {}  {}",
            color::gray(prefix),
            color::cyan(&entry.name),
            color::gray(status),
        );

        for (j, sd) in defs.iter().enumerate() {
            let is_last_def = j == defs.len() - 1;
            let branch = if is_last_def { "└── " } else { "├── " };
            let dn = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };
            println!("  {}  {}  {}",
                color::gray(&format!("{}    {}", connector, branch)),
                software::paint_software(dn, sd),
                color::gray(&format!("({})", sd.name)),
            );
        }

        println!("  {}    {}",
            connector,
            color::gray(&format!("地址: {}", entry.url)),
        );
    }
    Ok(())
}

fn run_info(name: &str) -> anyhow::Result<()> {
    let name_string = name.to_string();
    let source_root = paths::source_dir();
    let config_dir = paths::config_dir();
    let cfg = config::SourceConfig::new(config_dir);

    // 检查是否是社区源
    let entries = cfg.load();
    if let Some(entry) = entries.iter().find(|e| e.name == name) {
        let local_dir = paths::community_source_named(name);
        let count = count_json_files(&local_dir);
        let status = if entry.enabled { color::green("已启用") } else { color::gray("已禁用") };
        let defs = if local_dir.is_dir() {
            let mut defs: Vec<software::SoftwareDef> = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&local_dir) {
                for e in entries.flatten() {
                    let p = e.path();
                    if p.extension().map_or(false, |ext| ext == "json") && p.file_name().and_then(|n| n.to_str()) != Some("index.json") {
                        if let Ok(sd) = software::parse_json(&p) {
                            defs.push(sd);
                        }
                    }
                }
            }
            defs.sort_by(|a, b| a.name.cmp(&b.name));
            defs
        } else {
            Vec::new()
        };

        println!();
        println!("  {}  {}", color::bold_cyan(name), color::gray("社区源"));
        println!("  {}", color::gray("─".repeat(50)));
        println!("  {}  {}", color::gray("状态:"), status);
        println!("  {}  {}", color::gray("地址:"), color::gray(&entry.url));
        println!("  {}  {} 个软件定义", color::gray("软件数:"), color::yellow(&count.to_string()));

        if !defs.is_empty() {
            println!();
            println!("  {}", color::gray("软件列表:"));
            for sd in &defs {
                let dn = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };
                println!("    {}  {}",
                    software::paint_software(dn, sd),
                    color::gray(&format!("({})", sd.name)),
                );
            }
        }
        println!();
        return Ok(());
    }

    // 检查是否是内置分类源
    if let Some((_, label, _)) = paths::CATEGORY_META.iter().find(|(n, _, _)| *n == name_string) {
        let dir = source_root.join(&name_string);
        let count = count_json_files(&dir);
        let defs = if dir.is_dir() {
            let mut defs: Vec<software::SoftwareDef> = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for e in entries.flatten() {
                    let p = e.path();
                    if p.extension().map_or(false, |ext| ext == "json") && p.file_name().and_then(|n| n.to_str()) != Some("index.json") {
                        if let Ok(sd) = software::parse_json(&p) {
                            defs.push(sd);
                        }
                    }
                }
            }
            defs.sort_by(|a, b| a.name.cmp(&b.name));
            defs
        } else {
            Vec::new()
        };

        println!();
        println!("  {}  {}", color::bold_cyan(&name_string), color::gray(&format!("内置源 - {}", label)));
        println!("  {}", color::gray("─".repeat(50)));
        println!("  {}  {} 个软件定义", color::gray("软件数:"), color::yellow(&count.to_string()));
        if count > 0 {
            println!("  {}  {}", color::gray("更新方式:"), color::cyan("as source update"));
        }
        if !defs.is_empty() {
            println!();
            println!("  {}", color::gray("软件列表:"));
            for sd in &defs {
                let dn = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };
                println!("    {}  {}",
                    software::paint_software(dn, sd),
                    color::gray(&format!("({})", sd.name)),
                );
            }
        }
        println!();
        return Ok(());
    }

    // 检查是否是 tools
    let tools_dir = paths::tools_source_dir();
    let tools_count = count_json_files(&tools_dir);
    if name_string == "tools" {
        println!();
        println!("  {}  {}", color::bold_cyan("tools"), color::gray("内置源 - 自研工具"));
        println!("  {}", color::gray("─".repeat(50)));
        println!("  {}  {} 个工具定义", color::gray("工具数:"), color::yellow(&tools_count.to_string()));
        println!();
        return Ok(());
    }

    anyhow::bail!("未找到源 '{}'", name_string)
}

fn run_toggle(name: &str, enabled: bool) -> anyhow::Result<()> {
    let config_dir = paths::config_dir();
    let cfg = config::SourceConfig::new(config_dir);
    cfg.toggle(name, enabled)
}

// ── helpers ──────────────────────────────────────────────

fn count_files(dir: &std::path::Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count += count_files(&path);
            } else {
                count += 1;
            }
        }
    }
    count
}

fn count_json_files(dir: &std::path::Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") && path.file_name().and_then(|n| n.to_str()) != Some("index.json") {
                count += 1;
            }
        }
    }
    count
}
