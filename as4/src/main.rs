use clap::Parser;
use color::DisplayWidth;

mod cmd;
mod download;
mod installer;
mod paths;
mod software;

#[derive(Parser)]
#[clap(name = "as", version, about = "Windows 软件包管理器")]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Parser)]
enum Command {
    #[clap(name = "install", aliases = &["i"])]
    Install {
        names: Vec<String>,

        #[clap(short = 'p', long = "portable")]
        portable: bool,

        #[clap(short = 'I', long = "installer")]
        installer_force: bool,
    },
    #[clap(name = "uninstall")]
    Uninstall {
        name: String,
    },
    #[clap(name = "list", aliases = &["l"])]
    List {
        #[clap(short = 'a', long = "all")]
        all: bool,
    },
    #[clap(name = "show")]
    Show {
        name: String,
    },
    #[clap(name = "freeze")]
    Freeze,
    #[clap(name = "update")]
    Update,
}

fn main() {
    color::ansi::enable_ansi();

    match Cli::try_parse() {
        Ok(cli) => {
            if let Err(e) = run(cli.command) {
                eprintln!("{} {}", color::yellow("警告:"), e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            print_help(e.kind());
        }
    }
}

fn run(cmd: Command) -> anyhow::Result<()> {
    match cmd {
        Command::Install { names, portable, installer_force } => {
            cmd::install::run(names, portable, installer_force)
        }
        Command::Uninstall { name } => {
            cmd::uninstall::run(&name)
        }
        Command::List { all } => {
            cmd::list::run(all)
        }
        Command::Show { name } => {
            cmd::show::run(&name)
        }
        Command::Freeze => {
            cmd::freeze::run()
        }
        Command::Update => {
            cmd::update::run()
        }
    }
}

fn print_help(kind: clap::error::ErrorKind) {
    match kind {
        clap::error::ErrorKind::InvalidSubcommand => {
            eprintln!("{} 未知命令。可用命令:", color::red("错误"));
            print_commands();
        }
        _ => {
            print_commands();
        }
    }
}

fn print_commands() {
    println!();
    println!("  {}  {}", color::bold_cyan("as"), color::gray("Windows 软件包管理器"));
    println!();
    println!("  {}", color::bold_green("用法:"));
    println!("    {} {} {}", color::cyan("as"), color::yellow("<命令>"), color::gray("[参数]"));
    println!();
    println!("  {}", color::bold_green("命令:"));

    let cmds = [
        ("install",   "(i)",  "安装软件"),
        ("uninstall", "",     "卸载软件"),
        ("list",      "(l)",  "列出软件"),
        ("show",      "",     "查看软件详细信息"),
        ("freeze",    "",     "导出已安装清单"),
        ("update",    "",     "更新自身"),
    ];
    let cmd_w = cmds.iter().map(|(c, _, _)| c.display_width()).max().unwrap_or(10);
    let alias_w = cmds.iter().map(|(_, a, _)| a.display_width()).max().unwrap_or(4);

    for (cmd, alias, desc) in &cmds {
        let cmd_pad = pad(cmd, cmd_w + 2);
        let alias_display = if alias.is_empty() {
            " ".repeat(alias_w + 2)
        } else {
            format!(" {} ", pad(alias, alias_w))
        };
        println!("    {} {}{}", color::cyan(&cmd_pad), color::gray(&alias_display), desc);
    }

    println!();
}

fn pad(s: &str, w: usize) -> String {
    let cw = s.display_width();
    if cw >= w {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(w - cw))
    }
}