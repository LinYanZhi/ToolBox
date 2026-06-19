mod cmd_cache;
mod cmd_download;
mod cmd_downloader;
mod cmd_info;
mod cmd_init;
mod cmd_install;
mod cmd_list;
mod cmd_names;
mod cmd_self_update;
mod cmd_source;
mod cmd_tool;
mod cmd_uninstall;
mod downloader;
mod helpers;
mod help;
mod installer;
mod list_config;
mod opts;
mod paths;
mod pe_version;
mod registry;
mod repo;
mod software;
mod speedtest;

use clap::Parser;
use opts::*;

fn main() {
    color::enable_ansi();
    net::backend::set_tools_bin_dir(paths::tools_bin_dir());

    // ── 拦截无参数子命令，避免 clap 写 stderr（PowerShell 会变红） ──────────
    //
    // 规则：自动检测——子命令有 arg_required_else_help 或包含嵌套子命令，就拦截。
    //       纯靠 clap API（is_arg_required_else_help_set / has_subcommands），
    //       不需要手动维护列表，加新子命令后拦截自动生效。
    //
    // 拦截后先查白名单 print_custom_subcommand_help()（自定义样式），
    // 没注册白名单则 fallback 到 clap 帮助（带 styles() 颜色，不丑）。
    //
    // ⚠️  开发注意事项：
    //   - 子命令「所有参数可选」（如 cache、list），clap 不会报错，根本进不了这里。
    //     所以你想让 cache 显示自定义缓存列表，什么都不用改。
    //   - 子命令「需要参数」（如 install、info、uninstall），
    //     拦截后会先调 print_custom_subcommand_help()。
    //     如果想给它配自定义用法输出（而不是 clap 帮助），
    //     去它模块里加个 pub fn print_usage()，
    //     再在下方 print_custom_subcommand_help() 白名单注册一行即可。
    //   - 如果只想改个颜色/clap 帮助的样式，别来这里改——
    //     去 opts.rs 的 styles() 函数里调整，所有 clap 帮助统一生效。
    //   - 记得保持白名单按字母序排列，方便查找。
    {
        let args: Vec<String> = std::env::args().collect();
        if args.len() == 2 {
            let sub = &args[1].to_lowercase();
            let mut cmd = <Cli as clap::CommandFactory>::command();
            if let Some(subcmd) = cmd.find_subcommand_mut(&sub) {
                if subcmd.has_subcommands() || subcmd.is_arg_required_else_help_set() {
                    if !print_custom_subcommand_help(&sub) {
                        let _ = subcmd.print_help();
                        println!();
                    }
                    return;
                }
            }
        }
    }

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // 帮助/版本信息直接输出到 stdout（避免 clap 默认写 stderr 导致 PowerShell 变红）
            if e.kind() == clap::error::ErrorKind::DisplayHelp
                || e.kind() == clap::error::ErrorKind::DisplayVersion
            {
                let _ = e.print();
                std::process::exit(0);
            }
            let msg = translate_error(&e);
            eprintln!("  {}", color::bold_red(msg));
            std::process::exit(1);
        }
    };

    if cli.example {
        print_examples();
        return;
    }

    match cli.command {
        Some(cmd) => {
            let code = dispatch(cmd);
            std::process::exit(code);
        }
        None => {
            // 无子命令时显示帮助
            let mut cmd = <Cli as clap::CommandFactory>::command();
            let _ = cmd.print_help();
            println!();
        }
    }
}

/// 将 clap 错误翻译为中文（帮助/版本以外的错误）
fn translate_error(e: &clap::error::Error) -> String {
    use clap::error::ErrorKind;
    let raw_str = e.to_string();
    let raw = color::ansi::strip_ansi(&raw_str);
    match e.kind() {
        ErrorKind::UnknownArgument => {
            let flag = raw.split('\'').nth(1).unwrap_or("?");
            format!("未知的选项 '{}'\n提示: 使用 --help 查看可用选项", flag)
        }
        ErrorKind::InvalidSubcommand => {
            let sub = raw.split('\'').nth(1).unwrap_or("?");
            format!("未知的子命令 '{}'\n提示: 使用 --help 查看可用子命令", sub)
        }
        ErrorKind::MissingRequiredArgument => {
            "错误: 缺少必要参数\n提示: 使用 --help 查看正确用法".to_string()
        }
        _ => {
            raw
                .replace("error:", "错误:")
                .replace("tip:", "提示:")
                .replace("Usage:", "用法:")
                .replace("Commands:", "子命令:")
                .replace("Options:", "选项:")
                .replace("Arguments:", "参数:")
                .trim()
                .to_string()
        }
    }
}

fn dispatch(cmd: Commands) -> i32 {
    match cmd {
        Commands::Install(opts) => {
            help::run(|| cmd_install::run_install(opts))
        }
        Commands::List(opts) => {
            help::run(|| cmd_list::run_list(opts))
        }
        Commands::Info(opts) => {
            help::run(|| cmd_info::run_info(&opts.name, opts.urls))
        }
        Commands::Download(opts) => {
            help::run(|| cmd_download::run_download(opts))
        }
        Commands::Uninstall(opts) => {
            help::run(|| cmd_uninstall::run_uninstall(opts))
        }
        Commands::Cache(opts) => {
            help::run(|| cmd_cache::run_cache(opts.list, opts.clear, opts.open))
        }
        Commands::Source(cmd) => {
            help::run(|| cmd_source::run_source(&cmd))
        }
        Commands::Downloader(cmd) => {
            help::run(|| cmd_downloader::run_downloader(&cmd))
        }
        Commands::Tool(tool) => {
            dispatch_tool(tool)
        }
    }
}

fn dispatch_tool(tool: ToolCli) -> i32 {
    match tool {
        ToolCli::Init(opts) => {
            help::run(|| cmd_init::run_init(opts.global))
        }
        ToolCli::Add(opts) => {
            if opts.upgrade && opts.names.len() == 1 && opts.names[0].to_lowercase() == "as" {
                help::run(|| cmd_self_update::run_self_update())
            } else {
                help::run(|| cmd_tool::run_add(opts))
            }
        }
        ToolCli::List => {
            help::run(|| cmd_tool::run_list())
        }
        ToolCli::Remove(opts) => {
            help::run(|| cmd_tool::run_remove(&opts.name))
        }
    }
}

/// 无参数子命令的白名单分发表。
///
/// 返回 true 表示已处理，false 表示无自定义处理（调用方应 fallback 到 clap 帮助）。
///
/// ⚠️  开发注意事项：
///   - 这里是「白名单」——想自定义无参数输出的子命令才加到这里。
///     没加的不出 bug，只是 fallback 到 clap 帮助（带 styles() 颜色）。
///   - 添加新子命令时需要：
///     1. 在对应模块实现 `pub fn print_usage()`
///     2. 在此函数注册一行，如 `"xxx" => { cmd_xxx::print_usage(); true }`
///   - 保持按字母序排列，方便查找。
fn print_custom_subcommand_help(name: &str) -> bool {
    match name {
        "install" => { cmd_install::print_usage(); true }
        "info" => { cmd_info::print_usage(); true }
        "uninstall" => { cmd_uninstall::print_usage(); true }
        _ => false,
    }
}

fn print_examples() {
    let examples: Vec<(&str, &str, Vec<(&str, &str)>)> = vec![
        ("install", "安装指定软件", vec![
            (cmd_names::INSTALL, "安装 7-Zip（最新版）"),
            (cmd_names::INSTALL, "同时安装多个软件"),
            (cmd_names::INSTALL, "安装指定版本"),
            (cmd_names::INSTALL, "使用图形界面向导安装"),
            (cmd_names::INSTALL, "强制重新下载并安装"),
            (cmd_names::INSTALL, "仅下载，不安装"),
            (cmd_names::INSTALL, "指定安装类型为便携版"),
            (cmd_names::INSTALL, "检测更新，卸载旧版后安装新版"),
        ]),
        ("list", "列出已安装的软件", vec![
            (cmd_names::LIST, "仅列出已安装的软件"),
            (cmd_names::LIST, "列出全部（已安装 + 源中可用）"),
            (cmd_names::LIST, "按分类分组显示"),
            (cmd_names::LIST, "查看分类概览"),
            (cmd_names::LIST, "搜索名称、别名或描述"),
            (cmd_names::LIST, "按分类过滤"),
        ]),
        ("info", "查看软件详细信息", vec![
            (cmd_names::INFO, "查看 7-Zip 的详细信息"),
            (cmd_names::INFO, "查看 7-Zip 所有下载地址"),
        ]),
        ("download", "下载软件或文件", vec![
            (cmd_names::DOWNLOAD, "通过软件名称下载最新版"),
            (cmd_names::DOWNLOAD, "通过链接直接下载文件"),
            (cmd_names::DOWNLOAD, "打开下载目录"),
            (cmd_names::DOWNLOAD, "下载到指定目录"),
        ]),
        ("uninstall", "卸载指定软件", vec![
            ("as uninstall 7zip", "弹出卸载窗口卸载 7-Zip"),
            ("as uninstall 7zip --force", "强制删除（跳过卸载器）"),
        ]),
        ("cache", "管理下载缓存", vec![
            (cmd_names::CACHE, "列出缓存文件"),
            (cmd_names::CACHE_CLEAR, "清除所有缓存"),
            (cmd_names::CACHE_OPEN, "打开缓存目录"),
        ]),
        ("source", "管理软件源", vec![
            (cmd_names::SOURCE_UPDATE, "更新所有源"),
            (cmd_names::SOURCE_SPEEDTEST, "测速所有源"),
            (cmd_names::SOURCE_OPEN, "打开源目录"),
            (cmd_names::SOURCE_CLEAR, "清空所有源"),
        ]),
        ("downloader", "管理下载引擎后端", vec![
            (cmd_names::DOWNLOADER_LIST, "列出所有后端"),
            ("as downloader set curl on", "启用 curl"),
            ("as downloader set curl off", "禁用 curl"),
            (cmd_names::DOWNLOADER_OPEN, "打开配置目录"),
        ]),
        ("tool init", "初始化 as 环境", vec![
            (cmd_names::TOOL_INIT, "打印 tools/bin 加入 PATH 的配置提示"),
            ("as tool init -g", "写入用户 PATH 注册表"),
        ]),
        ("tool add", "安装/升级自研工具", vec![
            ("as tool add ls", "安装 ls 工具"),
            ("as tool add ls --upgrade", "升级 ls 工具"),
            ("as tool add as --upgrade", "升级 as 自身"),
        ]),
        ("tool", "管理自研工具", vec![
            (cmd_names::TOOL_LIST, "列出自研工具"),
            ("as tool remove ls", "移除 ls 工具"),
        ]),
    ];

    println!("{}", color::bold_green("使用示例"));
    println!();
    for group in examples {
        let (display, desc, entries) = group;
        println!("  {}   {}", color::bold_cyan(display), desc);
        for &(ex, help) in entries.iter() {
            println!("    {}  {}", color::bold(ex), color::gray(help));
        }
        println!();
    }
}
