use color::DisplayWidth;
use crate::paths;

/// 运行 `as downloader` 子命令。
pub fn run_downloader_list() -> anyhow::Result<()> {
    // 确保工具目录已设置
    net::backend::set_tools_bin_dir(paths::tools_bin_dir());
    let states = net::config::list_backend_states();
    if states.is_empty() {
        println!("  无法读取下载后端配置（配置文件可能损坏）");
        return Ok(());
    }

    // 收集每列数据
    struct Row { name: String, enabled: bool, available: bool, path_text: String, range: bool }
    let rows: Vec<Row> = states.iter().map(|(name, enabled, _threads)| {
        let available = net::backend::backend_is_available(name);
        let path_text = if net::backend::backend_is_builtin(name) {
            "[内置]".into()
        } else if let Some(p) = net::backend::backend_binary_path(name) {
            if p.starts_with("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\") {
                "powershell.exe".into()
            } else {
                p
            }
        } else {
            "未安装".into()
        };
        let range = net::backend::backend_supports_range(name);
        Row { name: name.clone(), enabled: *enabled, available, path_text, range }
    }).collect();

    // 计算列宽
    let name_w = rows.iter().map(|r| r.name.display_width()).max().unwrap_or(10);
    let path_w = rows.iter().map(|r| r.path_text.display_width()).max().unwrap_or(20).min(64);
    let label_name_w = name_w.max("名称".display_width());
    let label_status_w = 6usize.max("状态".display_width());
    let label_path_w = path_w.max("程序路径".display_width());
    let label_range_w = 6usize.max("分片".display_width());

    println!();
    println!("  {}",
        color::bold_cyan("下载后端列表"));
    println!("  {}",
        color::gray("(as downloader set <name> <on|off> 切换)"));
    println!();

    // 表头
    println!("    {}  {}  {}  {}",
        color::gray(color::pad_left("名称", label_name_w)),
        color::gray(color::pad_left("状态", label_status_w)),
        color::gray(color::pad_left("程序路径", label_path_w)),
        color::gray(color::pad_left("分片", label_range_w)),
    );
    // 分隔线
    let sep_w = label_name_w + label_status_w + label_path_w + label_range_w + 9;
    println!("    {}",
        color::gray(str::repeat("─", sep_w)));

    for r in &rows {
        // 先取纯文本，pad 后再加颜色
        let padded_status = if !r.enabled {
            color::red(&color::pad_left("禁用", label_status_w))
        } else if !r.available {
            color::yellow(&color::pad_left("未安装", label_status_w))
        } else {
            color::green(&color::pad_left("启用", label_status_w))
        };
        let padded_path = color::gray(&color::pad_left(&r.path_text, label_path_w));

        let padded_range = if r.range {
            color::green(&color::pad_left("支持", label_range_w))
        } else {
            color::red(&color::pad_left("不支持", label_range_w))
        };

        println!("    {}  {}  {}  {}",
            color::cyan(&color::pad_left(&r.name, label_name_w)),
            padded_status,
            padded_path,
            padded_range,
        );
    }

    println!();
    println!("  {}", color::gray(format!("配置文件: {}", net::config::config_file_path().to_string_lossy())));
    println!("  {}", color::gray("as downloader config --open  在资源管理器中打开"));
    println!();
    Ok(())
}

/// 设置后端启用/禁用。
pub fn run_downloader_set(name: &str, enable: bool) -> anyhow::Result<()> {
    let action = if enable { "启用" } else { "禁用" };
    net::config::set_backend_enabled(name, enable)?;
    println!("    {} 后端已{}（{}）",
        color::cyan(&net::config::find_backend_name(name)),
        color::green(action),
        color::gray(net::config::config_file_path().to_string_lossy()),
    );
    println!("  下次下载时将生效。\n");
    Ok(())
}

/// 显示配置路径或打开配置目录。
pub fn run_downloader_config(open: bool) -> anyhow::Result<()> {
    let path = net::config::config_file_path();
    let dir = paths::config_dir();

    if open {
        if dir.exists() {
            let _ = std::process::Command::new("explorer").arg(&dir).spawn();
            println!("    {} {}", color::green("已在资源管理器中打开:"), dir.display());
        } else {
            // 创建目录并打开
            std::fs::create_dir_all(&dir)?;
            let _ = std::process::Command::new("explorer").arg(&dir).spawn();
            println!("    {} {}", color::green("已创建并打开配置目录:"), dir.display());
        }
        return Ok(());
    }

    println!();
    println!("  {}", color::bold_cyan("下载引擎配置"));
    println!();
    println!("    {}", color::gray("路径:"));
    println!("      {}", path.display());
    println!();
    println!("    {}", color::gray("目录:"));
    println!("      {}", dir.display());
    println!();
    println!("    {}", color::gray("as downloader config --open  在资源管理器中打开"));
    println!();

    if !path.is_file() {
        println!("    {} 配置文件不存在，将使用默认配置运行。", color::yellow("提示:"));
        println!("    运行 {} 可创建默认配置文件。", color::cyan("as downloader list"));
        println!();
    }

    Ok(())
}
