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
    // 只清 apps、tools、community 的内容，保留目录结构
    for sub in &["apps", "tools", "community"] {
        let subdir = dir.join(sub);
        if subdir.exists() {
            let _ = std::fs::remove_dir_all(&subdir);
        }
        let _ = std::fs::create_dir_all(&subdir);
    }
    println!("已清空源目录（共 {} 个文件）: {}", total, dir.display());
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
    // 立即同步一次
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
    let builtin_dir = paths::source_dir();
    let builtin_apps = builtin_dir.join("apps");
    let builtin_tools = builtin_dir.join("tools");

    // 统计内置源
    let apps_count = count_json_files(&builtin_apps);
    let tools_count = count_json_files(&builtin_tools);

    println!();
    println!("  {}  {}", color::bold_cyan("软件源列表"), color::gray("(as source update 更新)"));
    println!("  {}", color::gray("─".repeat(50)));
    println!("  {}  {} 个软件定义", color::gray("内置源 (apps):"), apps_count);
    println!("  {}  {} 个工具定义", color::gray("内置源 (tools):"), tools_count);

    // 社区源
    let config_dir = paths::config_dir();
    let cfg = config::SourceConfig::new(config_dir);
    let entries = cfg.load();

    if entries.is_empty() {
        println!();
        println!("  暂无第三方社区源。");
        println!("  使用 {} 添加", color::yellow("as source add <名称> <仓库URL>"));
    } else {
        for entry in &entries {
            let status = if entry.enabled {
                color::green("启用")
            } else {
                color::gray("禁用")
            };
            let local_dir = paths::community_source_named(&entry.name);
            let count = count_json_files(&local_dir);
            println!();
            println!("  {} ({})", color::cyan(&entry.name), status);
            println!("    {}", color::gray(&entry.url));
            if count > 0 {
                println!("    {} {} 个软件定义", color::gray("本地缓存:"), count);
            } else {
                println!("    {} 尚未同步", color::gray("本地缓存:"));
            }
        }
    }

    println!();
    Ok(())
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
