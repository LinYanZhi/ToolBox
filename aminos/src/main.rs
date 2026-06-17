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

use cli::{App, CommandDef, ArgDef, ExampleGroup, ParseResult, ParsedArgs};

use opts::{DownloadOpts, InstallOpts, ListOpts, ToolAddOpts, UninstallOpts};

fn build_app() -> App {
    App::new("as", "轻量级 Windows 软件包管理器", "0.0.1")
        .command(
            CommandDef::new("install", "安装指定软件")
                .category("软件管理")
                .arg(ArgDef::positional("names", true, "软件名称（可同时指定多个）"))
                .arg(ArgDef::string(Some('v'), "version", "指定版本号"))
                .arg(ArgDef::flag(Some('g'), "gui", "使用图形界面安装（不静默）"))
                .arg(ArgDef::flag(Some('r'), "renew", "强制重新下载"))
                .arg(ArgDef::flag(Some('d'), "download-only", "仅下载，不安装"))
                .arg(ArgDef::string(None, "type", "安装类型：portable 或 installer"))
                .arg(ArgDef::flag(None, "upgrade", "检测更新，卸载旧版后安装新版"))
        )
        .command(
            CommandDef::new("list", "列出已安装的软件")
                .run_on_empty()
                .category("软件管理")
                .arg(ArgDef::flag(Some('a'), "all", "显示全部（已安装 + 源中可用）"))
                .arg(ArgDef::string(Some('f'), "filter", "按分类过滤"))
                .arg(ArgDef::string(Some('s'), "search", "搜索软件名、别名或描述"))
                .arg(ArgDef::flag(Some('d'), "downloaded", "仅显示已下载"))
                .arg(ArgDef::flag(None, "downloading", "仅显示下载中"))
                .arg(ArgDef::flag(None, "no-download", "仅显示未下载"))
                .arg(ArgDef::flag(Some('g'), "group", "按分类分组显示"))
                .arg(ArgDef::flag(None, "categories", "显示所有分类概览"))
        )
        .command(
            CommandDef::new("info", "查看软件详细信息")
                .category("软件管理")
                .arg(ArgDef::positional("name", true, "软件名称"))
                .arg(ArgDef::flag(Some('u'), "urls", "显示所有下载地址"))
        )
        .command(
            CommandDef::new("download", "下载软件或文件")
                .category("软件管理")
                .arg(ArgDef::positional("targets", false, "软件名称或下载链接"))
                .arg(ArgDef::flag(Some('o'), "open", "打开下载目录"))
                .arg(ArgDef::string(None, "target", "下载到指定目录"))
        )
        .command(
            CommandDef::new("uninstall", "卸载指定软件")
                .category("软件管理")
                .arg(ArgDef::positional("names", true, "软件名称（可同时指定多个）"))
                .arg(ArgDef::flag(Some('g'), "gui", "使用图形界面卸载"))
                .arg(ArgDef::flag(Some('f'), "force", "强制删除（跳过卸载器）"))
        )
        .command(
            CommandDef::new("cache", "管理下载缓存")
                .category("配置管理")
                .arg(ArgDef::flag(None, "list", "列出缓存文件（默认行为）"))
                .arg(ArgDef::flag(Some('c'), "clear", "清除所有缓存"))
                .arg(ArgDef::flag(Some('o'), "open", "在资源管理器中打开缓存目录"))
        )
        .command(
            CommandDef::new("source", "管理软件源")
                .category("配置管理")
                .arg(ArgDef::flag(Some('u'), "update", "更新所有源"))
                .arg(ArgDef::flag(Some('c'), "clear", "清空所有源"))
                .arg(ArgDef::flag(Some('o'), "open", "在资源管理器中打开源目录"))
                .arg(ArgDef::flag(None, "speedtest", "对源进行测速"))
                .arg(ArgDef::string(None, "name", "测速时指定软件[可选]"))
                .arg(ArgDef::flag(Some('S'), "software", "测速时以软件为单位统计"))
        )
        .command(
            CommandDef::new("downloader", "管理下载引擎后端")
                .category("配置管理")
                .arg(ArgDef::flag(None, "list", "列出所有下载后端"))
                .arg(ArgDef::flag(Some('o'), "open", "在资源管理器中打开配置目录"))
                .arg(ArgDef::positional("args", false, "子命令: set <名称> on|off"))
        )
        .command(
            CommandDef::new("tool", "管理自研工具（init/add/list/remove）")
                .category("工具管理")
                .subcommand(
                    CommandDef::new("init", "初始化环境（默认打印 PATH 提示，-g 写入注册表）")
                        .arg(ArgDef::flag(Some('g'), "global", "写入用户 PATH 注册表"))
                )
                .subcommand(
                    CommandDef::new("add", "安装/升级自研工具（--upgrade 升级模式）")
                        .arg(ArgDef::positional("names", true, "工具名称（可同时指定多个）"))
                        .arg(ArgDef::string(Some('v'), "version", "指定版本号"))
                        .arg(ArgDef::flag(Some('r'), "renew", "强制重新下载"))
                        .arg(ArgDef::flag(Some('d'), "download-only", "仅下载，不安装"))
                        .arg(ArgDef::flag(None, "upgrade", "升级模式"))
                )
                .subcommand(
                    CommandDef::new("list", "列出已安装的自研工具")
                )
                .subcommand(
                    CommandDef::new("remove", "移除一个自研工具")
                        .arg(ArgDef::positional("name", true, "工具名称"))
                )
        )
}

fn examples() -> Vec<ExampleGroup> {
    vec![
        ExampleGroup {
            command: "install",
            description: "安装指定软件",
            entries: vec![
                ("as install 7zip".into(), "安装 7-Zip（最新版）"),
                ("as install vscode python git".into(), "同时安装多个软件"),
                ("as install 7zip -v 1.0.0".into(), "安装指定版本"),
                ("as install 7zip --gui".into(), "使用图形界面向导安装"),
                ("as install 7zip --renew".into(), "强制重新下载并安装"),
                ("as install 7zip --download-only".into(), "仅下载，不安装"),
                ("as install 7zip --type portable".into(), "指定安装类型为便携版"),
                ("as install 7zip --upgrade".into(), "检测更新，卸载旧版后安装新版"),
            ],
        },
        ExampleGroup {
            command: "list",
            description: "列出已安装的软件",
            entries: vec![
                ("as list".into(), "仅列出已安装的软件"),
                ("as list -a".into(), "列出全部（已安装 + 源中可用）"),
                ("as list -g".into(), "按分类分组显示"),
                ("as list --categories".into(), "查看分类概览"),
                ("as list -s 压缩".into(), "搜索名称、别名或描述"),
                ("as list -f 开发工具".into(), "按分类过滤"),
            ],
        },
        ExampleGroup {
            command: "info",
            description: "查看软件详细信息",
            entries: vec![
                ("as info 7zip".into(), "查看 7-Zip 的详细信息"),
                ("as info 7zip --urls".into(), "查看 7-Zip 所有下载地址"),
            ],
        },
        ExampleGroup {
            command: "download",
            description: "下载软件或文件",
            entries: vec![
                ("as download 7zip".into(), "通过软件名称下载最新版"),
                ("as download <url>".into(), "通过链接直接下载文件"),
                ("as download -o".into(), "打开下载目录"),
                ("as download <url> --target ./tmp".into(), "下载到指定目录"),
            ],
        },
        ExampleGroup {
            command: "uninstall",
            description: "卸载指定软件",
            entries: vec![
                ("as uninstall 7zip".into(), "静默卸载 7-Zip"),
                ("as uninstall vscode python".into(), "同时卸载多个软件"),
                ("as uninstall 7zip --gui".into(), "使用图形界面卸载向导"),
                ("as uninstall 7zip --force".into(), "强制删除（跳过卸载器）"),
            ],
        },
        ExampleGroup {
            command: "cache",
            description: "管理下载缓存",
            entries: vec![
                ("as cache".into(), "列出缓存文件"),
                ("as cache -c".into(), "清除所有缓存"),
                ("as cache -o".into(), "打开缓存目录"),
            ],
        },
        ExampleGroup {
            command: "source",
            description: "管理软件源",
            entries: vec![
                ("as source -u".into(), "更新所有源"),
                ("as source --speedtest".into(), "测速所有源"),
                ("as source -o".into(), "打开源目录"),
                ("as source -c".into(), "清空所有源"),
            ],
        },
        ExampleGroup {
            command: "downloader",
            description: "管理下载引擎后端",
            entries: vec![
                ("as downloader --list".into(), "列出所有后端"),
                ("as downloader set curl on".into(), "启用 curl"),
                ("as downloader set curl off".into(), "禁用 curl"),
                ("as downloader -o".into(), "打开配置目录"),
            ],
        },
        ExampleGroup {
            command: "tool init",
            description: "初始化 as 环境",
            entries: vec![
                ("as tool init".into(), "打印 tools/bin 加入 PATH 的配置提示"),
                ("as tool init -g".into(), "写入用户 PATH 注册表"),
            ],
        },
        ExampleGroup {
            command: "tool add",
            description: "安装/升级自研工具",
            entries: vec![
                ("as tool add ls".into(), "安装 ls 工具"),
                ("as tool add ls --upgrade".into(), "升级 ls 工具"),
                ("as tool add as --upgrade".into(), "升级 as 自身"),
            ],
        },
        ExampleGroup {
            command: "tool",
            description: "管理自研工具",
            entries: vec![
                ("as tool init".into(), "初始化环境"),
                ("as tool list".into(), "列出自研工具"),
                ("as tool remove ls".into(), "移除 ls 工具"),
            ],
        },
    ]
}

fn main() {
    color::enable_ansi();
    net::backend::set_tools_bin_dir(paths::tools_bin_dir());

    let app = build_app();
    let raw_args: Vec<String> = std::env::args().collect();

    // 处理 -e/--example
    if raw_args.iter().any(|a| a == "-e" || a == "--example") {
        app.print_examples(&examples());
        return;
    }

    // 处理 -V/--version
    if raw_args.iter().any(|a| a == "-V" || a == "--version") {
        println!("as {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    // 解析命令
    let args: Vec<String> = raw_args.into_iter().skip(1).collect();

    match app.parse(&args) {
        Ok(ParseResult::Executed(parsed, cmd)) => {
            if parsed.subcommand_path.is_empty() {
                dispatch_root(&parsed, cmd, &app);
            } else {
                dispatch_sub(&parsed, cmd, &app);
            }
        }
        Ok(ParseResult::ShowHelp(cmd, path)) => {
            app.print_command_help(cmd, &path);
        }
        Err(e) => {
            app.print_error(&e);
        }
    }
}

fn dispatch_root(parsed: &ParsedArgs, cmd: &cli::CommandDef, app: &App) {
    match parsed.command.as_str() {
        "install" => {
            let names = parsed.all().to_vec();
            let version = parsed.get_string("version").map(|s| s.to_string());
            let gui = parsed.flag("gui");
            let renew = parsed.flag("renew");
            let download_only = parsed.flag("download-only");
            let inst_type = parsed.get_string("type").map(|s| s.to_string());
            let upgrade = parsed.flag("upgrade");
            let opts = InstallOpts::new(names, version, gui, renew, download_only, inst_type, upgrade);
            let _ = help::run(|| cmd_install::run_install(opts));
        }
        "list" => {
            let all = parsed.flag("all");
            let filter = parsed.get_string("filter").map(|s| s.to_string());
            let search = parsed.get_string("search").map(|s| s.to_string());
            let downloaded = parsed.flag("downloaded");
            let downloading = parsed.flag("downloading");
            let no_download = parsed.flag("no-download");
            let group = parsed.flag("group");
            let categories = parsed.flag("categories");
            let opts = ListOpts::new(all, filter, search, downloaded, downloading, no_download, group, categories);
            let _ = help::run(|| cmd_list::run_list(opts));
        }
        "info" => {
            let name = parsed.first().unwrap_or("");
            let urls = parsed.flag("urls");
            let _ = help::run(|| cmd_info::run_info(name, urls));
        }
        "download" => {
            let targets = parsed.all().to_vec();
            let open = parsed.flag("open");
            let target_dir = parsed.get_string("target").map(|s| s.to_string());
            let opts = DownloadOpts::new(targets, open, target_dir);
            let _ = help::run(|| cmd_download::run_download(opts));
        }
        "uninstall" => {
            let names = parsed.all().to_vec();
            let gui = parsed.flag("gui");
            let force = parsed.flag("force");
            let opts = UninstallOpts::new(names, gui, force);
            let _ = help::run(|| cmd_uninstall::run_uninstall(opts));
        }
        "cache" => {
            let list = parsed.flag("list");
            let clear = parsed.flag("clear");
            let open = parsed.flag("open");
            let _ = help::run(|| cmd_cache::run_cache(list, clear, open));
        }
        "source" => {
            let update = parsed.flag("update");
            let clear = parsed.flag("clear");
            let open = parsed.flag("open");
            let speedtest = parsed.flag("speedtest");
            let names = parsed.get_string("name").map(|s| vec![s.to_string()]).unwrap_or_default();
            let software_flag = parsed.flag("software");
            let _ = help::run(|| cmd_source::run_source(update, clear, open, speedtest, names, software_flag));
        }
        "downloader" => {
            let list = parsed.flag("list");
            let open = parsed.flag("open");
            let args = parsed.all();
            let set = if args.len() >= 3 && args[0] == "set" {
                Some(vec![args[1].clone(), args[2].clone()])
            } else {
                None
            };
            let _ = help::run(|| cmd_downloader::run_downloader(list, set, open));
        }
        "tool" => {
            // tool without subcommand → show subcommand help
            app.print_command_help(cmd, &[]);
        }
        _ => {}
    }
}

fn dispatch_sub(parsed: &ParsedArgs, _cmd: &cli::CommandDef, _app: &App) {
    let parent = parsed.subcommand_path.first().map(|s| s.as_str()).unwrap_or("");
    match (parent, parsed.command.as_str()) {
        ("tool", "init") => {
            let global = parsed.flag("global");
            let _ = help::run(|| cmd_init::run_init(global));
        }
        ("tool", "add") => {
            let names = parsed.all().to_vec();
            let version = parsed.get_string("version").map(|s| s.to_string());
            let renew = parsed.flag("renew");
            let download_only = parsed.flag("download-only");
            let upgrade = parsed.flag("upgrade");

            // 特例：as tool add as --upgrade → 更新 as 自身
            if upgrade && names.len() == 1 && names[0].to_lowercase() == "as" {
                let _ = help::run(|| cmd_self_update::run_self_update());
            } else {
                let opts = ToolAddOpts::new(names, version, renew, download_only, upgrade);
                let _ = help::run(|| cmd_tool::run_add(opts));
            }
        }
        ("tool", "list") => {
            let _ = help::run(|| cmd_tool::run_list());
        }
        ("tool", "remove") => {
            let name = parsed.first().unwrap_or("");
            let _ = help::run(|| cmd_tool::run_remove(name));
        }
        _ => {}
    }
}
