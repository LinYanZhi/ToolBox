/// pp — 轻量 PATH 显示工具，根据硬编码规则给路径着色

use clap::{Parser, CommandFactory, builder::styling};

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

struct Rule {
    pattern: &'static str,
    color: &'static str,
    style: &'static str,
}

/// PATH 着色规则（源自原 pp.yaml 配置）
static PATH_RULES: &[Rule] = &[
    Rule { pattern: r"C:\Windows*", color: "gray", style: "" },
    Rule { pattern: r"*\Notepad_file\*", color: "lightgreen", style: "" },
    Rule { pattern: r"*\Program Files\Git\cmd", color: "yellow", style: "" },
    Rule { pattern: r"*\AppData\Local\Microsoft\WindowsApps", color: "yellow", style: "" },
    Rule { pattern: r"*\Scripts", color: "lightyellow", style: "" },
    Rule { pattern: r"*\AppData\Local\Programs\Python\*", color: "lightyellow", style: "" },
    Rule { pattern: r"*\Anaconda3\envs\*", color: "green", style: "" },
    Rule { pattern: r"*\Anaconda3\condabin", color: "green", style: "" },
    Rule { pattern: r"*\miniconda3\condabin", color: "green", style: "" },
    Rule { pattern: r"*\AppData\Roaming\nvm", color: "lightcyan", style: "" },
    Rule { pattern: r"*\AppData\Local\nvm", color: "lightcyan", style: "" },
    Rule { pattern: r"C:\Program Files\nodejs", color: "cyan", style: "" },
    Rule { pattern: r"*\nvm_nodejs", color: "cyan", style: "" },
    Rule { pattern: r"*\nvm\*", color: "cyan", style: "" },
    Rule { pattern: r"*\mysql-8.0.35-winx64\bin", color: "lightblue", style: "" },
    Rule { pattern: r"*\Program Files\ShadowBot", color: "lightred", style: "" },
    Rule { pattern: r"\\*", color: "", style: "underline" },
];

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
        print_styles();
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
            let (color, style) = match_path(entry);
            if color.is_empty() && style.is_empty() {
                println!("{entry}");
            } else {
                println!("{}", styled(entry, color, style));
            }
        }
    }
}

/// 显示 PATH 着色规则预览
fn print_styles() {
    use color::*;
    println!("{}", bold_cyan("PATH 着色规则预览"));
    println!("{}", gray("━".repeat(40)));
    println!();
    for rule in PATH_RULES {
        let sample = if rule.pattern.contains('*') {
            // 把通配符替换为有意义的示例文本
            rule.pattern
                .replace(r"C:\Windows*", r"C:\Windows\System32")
                .replace(r"*\Notepad_file\*", r"D:\My\Notepad_file\scripts")
                .replace(r"*\AppData\Local\Microsoft\WindowsApps", r"C:\Users\Me\AppData\Local\Microsoft\WindowsApps")
                .replace(r"*\Scripts", r"C:\Python312\Scripts")
                .replace(r"*\AppData\Local\Programs\Python\*", r"C:\Users\Me\AppData\Local\Programs\Python\312")
                .replace(r"*\Anaconda3\envs\*", r"C:\Users\Me\Anaconda3\envs\base")
                .replace(r"*\Anaconda3\condabin", r"C:\Users\Me\Anaconda3\condabin")
                .replace(r"*\miniconda3\condabin", r"C:\Users\Me\miniconda3\condabin")
                .replace(r"*\AppData\Roaming\nvm", r"C:\Users\Me\AppData\Roaming\nvm")
                .replace(r"*\AppData\Local\nvm", r"C:\Users\Me\AppData\Local\nvm")
                .replace(r"C:\Program Files\nodejs", r"C:\Program Files\nodejs")
                .replace(r"*\nvm_nodejs", r"C:\ProgramData\nvm_nodejs")
                .replace(r"*\nvm\*", r"C:\Users\Me\nvm\versions")
                .replace(r"*\mysql-8.0.35-winx64\bin", r"D:\mysql-8.0.35-winx64\bin")
                .replace(r"*\Program Files\ShadowBot", r"C:\Program Files\ShadowBot")
                .replace(r"\\*", r"\\server\share\folder")
        } else {
            rule.pattern.to_string()
        };
        let (c, s) = if rule.color.is_empty() && rule.style.is_empty() {
            ("", "")
        } else {
            (rule.color, rule.style)
        };
        println!("  {}  {}", styled(&sample, c, s), gray(&format!("  ← {}", rule.pattern)));
    }
    println!();
    println!("{}", gray("无颜色的路径保持默认终端颜色"));
}

fn match_path(text: &str) -> (&'static str, &'static str) {
    for rule in PATH_RULES {
        if !rule.pattern.contains('*') && rule.pattern.eq_ignore_ascii_case(text) {
            return (rule.color, rule.style);
        }
    }
    for rule in PATH_RULES {
        if rule.pattern.contains('*') && wildmatch(rule.pattern, text) {
            return (rule.color, rule.style);
        }
    }
    ("", "")
}

fn wildmatch(pattern: &str, text: &str) -> bool {
    let p = pattern.to_lowercase();
    let t = text.to_lowercase();
    let parts: Vec<&str> = p.split('*').collect();
    if parts.is_empty() || (parts.len() == 1 && parts[0].is_empty()) {
        return true;
    }
    if !parts[0].is_empty() && !t.starts_with(parts[0]) {
        return false;
    }
    let last = parts.last().unwrap();
    if !last.is_empty() && !t.ends_with(last) {
        return false;
    }
    let mut pos = parts[0].len();
    for i in 1..parts.len() - 1 {
        let part = parts[i];
        if part.is_empty() {
            continue;
        }
        match t[pos..].find(part) {
            Some(idx) => pos += idx + part.len(),
            None => return false,
        }
    }
    true
}

fn color_code(name: &str) -> Option<u8> {
    match name.to_lowercase().as_str() {
        "black" => Some(30), "red" => Some(31), "green" => Some(32),
        "yellow" => Some(33), "blue" => Some(34), "magenta" | "purple" => Some(35),
        "cyan" => Some(36), "white" => Some(37), "gray" => Some(90),
        "lightred" => Some(91), "lightgreen" => Some(92), "lightyellow" => Some(93),
        "lightblue" => Some(94), "lightmagenta" | "lightpurple" => Some(95),
        "lightcyan" => Some(96), "brightwhite" => Some(97),
        _ => None,
    }
}

fn style_code(name: &str) -> Option<u8> {
    match name.to_lowercase().as_str() {
        "bold" => Some(1), "dim" => Some(2), "italic" => Some(3),
        "underline" => Some(4), "blink" => Some(5), "reverse" => Some(7),
        "hidden" => Some(8), "strikethrough" => Some(9),
        _ => None,
    }
}

fn styled(text: &str, color: &str, style: &str) -> String {
    let mut codes: Vec<u8> = Vec::new();
    if let Some(c) = color_code(color) { codes.push(c); }
    if let Some(s) = style_code(style) { codes.push(s); }
    if codes.is_empty() {
        return text.to_string();
    }
    let code_str = codes.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(";");
    format!("\x1b[{code_str}m{text}\x1b[0m")
}
