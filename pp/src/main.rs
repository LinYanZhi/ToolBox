/// pp — 轻量 PATH 显示工具，根据硬编码规则给路径着色

use clap::{Parser, CommandFactory, builder::styling};
use path_rules;

fn styles() -> styling::Styles {
    styling::Styles::styled()
        .header(styling::AnsiColor::Green.on_default().bold())
        .usage(styling::AnsiColor::Green.on_default().bold())
        .literal(styling::AnsiColor::Cyan.on_default().bold())
        .placeholder(styling::AnsiColor::Yellow.on_default().italic())
        .error(styling::AnsiColor::Red.on_default().bold())
        .valid(styling::AnsiColor::Cyan.on_default().bold())
        .invalid(styling::AnsiColor::Yellow.on_default())
}

#[derive(Parser)]
#[command(
    name = "pp",
    version = "0.0.1",
    about = "PATH 环境变量查看器，支持颜色区分不同路径类型",
    styles = styles(),
    color = clap::ColorChoice::Always,
    arg_required_else_help = false,
    disable_help_flag = true,
    disable_version_flag = true,
)]
struct Cli {
    /// 无颜色输出
    #[arg(short = 'n', long = "no-color")]
    no_color: bool,

    /// 显示着色规则预览
    #[arg(short = 's', long = "style")]
    style: bool,

    /// 显示帮助信息
    #[arg(short = 'h', long = "help", global = true)]
    help: bool,

    /// 显示版本号
    #[arg(short = 'V', long = "version", global = true)]
    version: bool,
}

fn main() {
    color::enable_ansi();

    let cli = Cli::parse();

    if cli.help {
        let cmd = <Cli as CommandFactory>::command();
        let _ = cmd.next_help_heading("选项:").print_help();
        println!();
        return;
    }

    if cli.version {
        println!("pp 0.0.1");
        return;
    }

    if cli.style {
        path_rules::print_path_styles();
        return;
    }

    let no_color = cli.no_color;

    let path = match std::env::var("PATH") {
        Ok(p) => p,
        Err(_) => {
            eprintln!("错误: 无法读取 PATH 环境变量");
            std::process::exit(1);
        }
    };

    println!();
    for entry in path.split(';') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        if no_color {
            println!("{entry}");
        } else {
            let (color, style) = path_rules::match_path(entry);
            if color.is_empty() && style.is_empty() {
                println!("{entry}");
            } else {
                println!("{}", path_rules::styled(entry, color, style));
            }
        }
    }
}
