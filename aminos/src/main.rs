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

    // 提前处理 --version，避免和子命令的 ver 参数冲突
    let raw_args: Vec<String> = std::env::args().collect();
    let has_version = if raw_args.len() == 2 {
        raw_args[1] == "--version" || raw_args[1] == "-V"
    } else {
        false
    };

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            let msg = translate_clap_error(&e);
            eprintln!("  {}", color::bold_red(msg));
            std::process::exit(1);
        }
    };

    if cli.version_flag || has_version {
        println!("as {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if cli.help {
        let cmd = <Cli as clap::CommandFactory>::command();
        let _ = cmd
            .subcommand_help_heading("子命令:")
            .print_help();
        println!();
        return;
    }

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

/// 将 clap 的错误信息翻译为中文
fn translate_clap_error(e: &clap::error::Error) -> String {
    use clap::error::ErrorKind;
    let msg = e.to_string();

    match e.kind() {
        ErrorKind::InvalidSubcommand => {
            // 从原始错误中提取子命令名和提示
            let mut subcmd = String::new();
            let mut tip = String::new();
            // 用 strip_ansi 拿到纯文本
            let clean = color::ansi::strip_ansi(&msg);
            for line in clean.lines() {
                let line = line.trim();
                if line.contains("unrecognized subcommand") || line.contains("subcommand") {
                    // 提取单引号内的子命令名
                    if let Some(start) = line.find('\'') {
                        let rest = &line[start + 1..];
                        if let Some(end) = rest.find('\'') {
                            subcmd = rest[..end].to_string();
                        }
                    }
                } else if line.starts_with("tip:") || line.starts_with("提示:") {
                    tip = line
                        .replace("tip:", "提示:")
                        .replace("some similar subcommands exist", "相似的子命令");
                }
            }
            if !subcmd.is_empty() {
                format!("未知的子命令 '{}'\n{}", subcmd, tip)
            } else {
                clean.to_string()
            }
        }
        ErrorKind::MissingRequiredArgument => {
            let clean = color::ansi::strip_ansi(&msg);
            let mut translated = String::new();
            for line in clean.lines() {
                let line = line.trim();
                if line.starts_with("error:") {
                    translated.push_str("错误: 缺少必要参数");
                    translated.push('\n');
                } else {
                    translated.push_str(line);
                    translated.push('\n');
                }
            }
            translated.trim().to_string()
        }
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
            msg
        }
        _ => {
            let clean = color::ansi::strip_ansi(&msg);
            clean
                .replace("error:", "错误:")
                .replace("tip:", "提示:")
                .replace("Usage:", "用法:")
                .replace("For more information, try '--help'.", "更多信息请查看 --help")
                .replace("Commands:", "子命令:")
                .replace("Options:", "选项:")
                .replace("Arguments:", "参数:")
                .replace("Flags:", "标志:")
                .replace("Subcommand:", "子命令:")
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
        Commands::Source(opts) => {
            help::run(|| cmd_source::run_source(
                opts.update,
                opts.clear,
                opts.open,
                opts.speedtest,
                opts.name.map(|n| vec![n]).unwrap_or_default(),
                opts.software,
            ))
        }
        Commands::Downloader(opts) => {
            let set = if opts.args.len() >= 3 && opts.args[0] == "set" {
                Some(vec![opts.args[1].clone(), opts.args[2].clone()])
            } else {
                None
            };
            help::run(|| cmd_downloader::run_downloader(opts.list, set, opts.open))
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

fn print_examples() {
    let examples = vec![
        ("install", "安装指定软件", vec![
            ("as install 7zip", "安装 7-Zip（最新版）"),
            ("as install vscode python git", "同时安装多个软件"),
            ("as install 7zip -v 1.0.0", "安装指定版本"),
            ("as install 7zip --gui", "使用图形界面向导安装"),
            ("as install 7zip --renew", "强制重新下载并安装"),
            ("as install 7zip --download-only", "仅下载，不安装"),
            ("as install 7zip --type portable", "指定安装类型为便携版"),
            ("as install 7zip --upgrade", "检测更新，卸载旧版后安装新版"),
        ]),
        ("list", "列出已安装的软件", vec![
            ("as list", "仅列出已安装的软件"),
            ("as list -a", "列出全部（已安装 + 源中可用）"),
            ("as list -g", "按分类分组显示"),
            ("as list --categories", "查看分类概览"),
            ("as list -s 压缩", "搜索名称、别名或描述"),
            ("as list -f 开发工具", "按分类过滤"),
        ]),
        ("info", "查看软件详细信息", vec![
            ("as info 7zip", "查看 7-Zip 的详细信息"),
            ("as info 7zip --urls", "查看 7-Zip 所有下载地址"),
        ]),
        ("download", "下载软件或文件", vec![
            ("as download 7zip", "通过软件名称下载最新版"),
            ("as download <url>", "通过链接直接下载文件"),
            ("as download -o", "打开下载目录"),
            ("as download <url> --target ./tmp", "下载到指定目录"),
        ]),
        ("uninstall", "卸载指定软件", vec![
            ("as uninstall 7zip", "静默卸载 7-Zip"),
            ("as uninstall vscode python", "同时卸载多个软件"),
            ("as uninstall 7zip --gui", "使用图形界面卸载向导"),
            ("as uninstall 7zip --force", "强制删除（跳过卸载器）"),
        ]),
        ("cache", "管理下载缓存", vec![
            ("as cache", "列出缓存文件"),
            ("as cache -c", "清除所有缓存"),
            ("as cache -o", "打开缓存目录"),
        ]),
        ("source", "管理软件源", vec![
            ("as source -u", "更新所有源"),
            ("as source --speedtest", "测速所有源"),
            ("as source -o", "打开源目录"),
            ("as source -c", "清空所有源"),
        ]),
        ("downloader", "管理下载引擎后端", vec![
            ("as downloader --list", "列出所有后端"),
            ("as downloader set curl on", "启用 curl"),
            ("as downloader set curl off", "禁用 curl"),
            ("as downloader -o", "打开配置目录"),
        ]),
        ("tool init", "初始化 as 环境", vec![
            ("as tool init", "打印 tools/bin 加入 PATH 的配置提示"),
            ("as tool init -g", "写入用户 PATH 注册表"),
        ]),
        ("tool add", "安装/升级自研工具", vec![
            ("as tool add ls", "安装 ls 工具"),
            ("as tool add ls --upgrade", "升级 ls 工具"),
            ("as tool add as --upgrade", "升级 as 自身"),
        ]),
        ("tool", "管理自研工具", vec![
            ("as tool init", "初始化环境"),
            ("as tool list", "列出自研工具"),
            ("as tool remove ls", "移除 ls 工具"),
        ]),
    ];

    println!("{}", color::bold_green("使用示例"));
    println!();
    for (cmd, desc, entries) in examples {
        println!("  {}   {}", color::bold_cyan(cmd), desc);
        for (example_msg, help_msg) in entries {
            println!("    {}  {}", color::bold(example_msg), color::gray(help_msg));
        }
        println!();
    }
}
