/// ss — 轻量环境变量显示工具，根据硬编码规则给变量着色

use std::collections::BTreeMap;

use clap::{Parser, Subcommand, builder::styling};
use color;
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
    name = "ss",
    version = "0.0.1",
    about = "环境变量查看器，支持颜色区分、左对齐，支持打开系统环境变量对话框",
    styles = styles(),
    color = clap::ColorChoice::Always,
    arg_required_else_help = false,
)]
struct Cli {
    /// 左对齐变量名
    #[arg(short = 'l', long = "left")]
    left: bool,

    /// 无颜色输出
    #[arg(short = 'n', long = "no-color")]
    no_color: bool,

    /// 显示着色规则预览
    #[arg(short = 's', long = "style")]
    style: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// 打开系统环境变量对话框
    Gui {
        /// 以管理员权限打开（提权）
        #[arg(short = 'r', long = "root")]
        root: bool,
    },
}

struct Rule {
    name: &'static str,
    color: &'static str,
    style: &'static str,
}

/// PATH 着色规则（与 pp 保持一致 — 通过 path-rules 共享库）
static VAR_RULES: &[Rule] = &[
    Rule { name: "SYSTEMDRIVE", color: "gray", style: "" },
    Rule { name: "SYSTEMROOT", color: "gray", style: "" },
    Rule { name: "TEMP", color: "gray", style: "" },
    Rule { name: "TMP", color: "gray", style: "" },
    Rule { name: "USERNAME", color: "gray", style: "" },
    Rule { name: "USERPROFILE", color: "gray", style: "" },
    Rule { name: "COMSPEC", color: "gray", style: "" },
    Rule { name: "PROMPT", color: "gray", style: "" },
    Rule { name: "CONDA_HOME", color: "green", style: "" },
    Rule { name: "MYSQL_HOME", color: "lightblue", style: "" },
    Rule { name: "NVM_HOME", color: "lightcyan", style: "" },
    Rule { name: "NVM_SYMLINK", color: "lightcyan", style: "" },
    Rule { name: "ENV", color: "purple", style: "" },
    Rule { name: "ENV_PATH", color: "purple", style: "" },
    Rule { name: "KMP_DUPLICATE_LIB_OK", color: "purple", style: "" },
    Rule { name: "REDIS_HOME", color: "red", style: "" },
    Rule { name: "PYTHON_HOME", color: "lightyellow", style: "" },
    Rule { name: "JAVA_HOME", color: "blue", style: "" },
    Rule { name: "GOPATH", color: "blue", style: "" },
    Rule { name: "PATH", color: "green", style: "bold" },
    Rule { name: "SHADOWBOT_CULTURE", color: "lightred", style: "" },
    Rule { name: "SHADOWBOT_ROOT_X64", color: "lightred", style: "" },
    Rule { name: "RUSTUP_UPDATE_ROOT", color: "lightpurple", style: "" },
    Rule { name: "RUSTUP_DIST_SERVER", color: "lightpurple", style: "" },
    Rule { name: "WT_SESSION", color: "", style: "italic" },
];

/// 需要排除（不显示）的变量
static EXCLUDE_NAMES: &[&str] = &[
    "_PYI_APPLICATION_HOME_DIR", "_PYI_ARCHIVE_FILE", "_PYI_PARENT_PROCESS_LEVEL",
    "_OLD_VIRTUAL_PATH", "_OLD_VIRTUAL_PROMPT",
    "_MY_CURRENT_ENV", "_MY_ENV_ACTIVATED", "_MY_OLD_PATH", "_MY_OLD_PROMPT", "_MY_FORCE",
];

fn main() {
    color::enable_ansi();

    let cli = Cli::parse();

    if cli.style {
        print_styles();
        return;
    }

    let no_color = cli.no_color;
    let left_align = cli.left;

    if let Some(cmd) = &cli.command {
        match cmd {
            Commands::Gui { root: true } => {
                // 提权打开
                let _ = std::process::Command::new("powershell")
                    .args([
                        "-NoProfile",
                        "-Command",
                        "Start-Process -FilePath 'rundll32.exe' -ArgumentList 'sysdm.cpl,EditEnvironmentVariables' -Verb RunAs",
                    ])
                    .spawn();
                return;
            }
            Commands::Gui { root: false } => {
                // 普通打开
                let _ = std::process::Command::new("rundll32.exe")
                    .args(["sysdm.cpl,EditEnvironmentVariables"])
                    .spawn();
                return;
            }
        }
    }

    let mut vars: BTreeMap<String, String> = BTreeMap::new();
    for (k, v) in std::env::vars() {
        if k.starts_with('=') { continue; }
        if is_excluded(&k) { continue; }
        vars.insert(k, v);
    }

    if vars.is_empty() { return; }

    let max_len = vars.keys().map(|k| k.len()).max().unwrap_or(0);
    let prefix_width = max_len + 3;
    let indent = " ".repeat(prefix_width);

    println!();
    for (name, value) in &vars {
        let (color, style) = match_var(name);
        let is_path = name.eq_ignore_ascii_case("PATH");
        let spaced = if left_align {
            format!("{name}{}", " ".repeat(max_len - name.len()))
        } else {
            format!("{}{name}", " ".repeat(max_len - name.len()))
        };

        if is_path && !no_color {
            // PATH 变量的值按 pp 的规则逐段着色
            let segments: Vec<&str> = value.split(';').collect();
            for (i, seg) in segments.iter().enumerate() {
                let seg = seg.trim();
                if seg.is_empty() { continue; }
                let (p_color, p_style) = path_rules::match_path(seg);
                let colored = if p_color.is_empty() && p_style.is_empty() {
                    seg.to_string()
                } else {
                    path_rules::styled(seg, p_color, p_style)
                };
                let line = if i == segments.len() - 1 || !value.contains(';') {
                    colored
                } else {
                    format!("{colored};")
                };
                if i == 0 {
                    println!("{} = {}", path_rules::styled(&spaced, color, style), line);
                } else {
                    println!("{indent}{line}");
                }
            }
        } else if value.contains(';') {
            let segments: Vec<&str> = value.split(';').filter(|s| !s.is_empty()).collect();
            for (i, seg) in segments.iter().enumerate() {
                let line = if i == segments.len() - 1 {
                    seg.to_string()
                } else {
                    format!("{seg};")
                };
                if i == 0 {
                    if no_color {
                        println!("{spaced} = {line}");
                    } else {
                        println!("{} = {}", path_rules::styled(&spaced, color, style), path_rules::styled(&line, color, style));
                    }
                } else {
                    if no_color {
                        println!("{indent}{line}");
                    } else {
                        println!("{}{}", indent, path_rules::styled(&line, color, style));
                    }
                }
            }
        } else {
            if no_color {
                println!("{spaced} = {value}");
            } else {
                println!("{} = {}", path_rules::styled(&spaced, color, style), path_rules::styled(value, color, style));
            }
        }
    }
}

fn is_excluded(name: &str) -> bool {
    let lower = name.to_lowercase();
    EXCLUDE_NAMES.iter().any(|e| e.to_lowercase() == lower)
}

/// 显示着色规则预览
fn print_styles() {
    use color::*;
    println!("{}", bold_cyan("环境变量着色规则预览"));
    println!("{}", gray("━".repeat(40)));
    println!();
    println!("{}", bold_green("变量名规则:"));
    for rule in VAR_RULES {
        let (c, s) = (rule.color, rule.style);
        let preview = if c.is_empty() && s.is_empty() {
            gray("颜色").to_string()
        } else {
            path_rules::styled(&rule.name, c, s)
        };
        let desc = if !c.is_empty() {
            gray(if !s.is_empty() { format!("  ← {}, {}", c, s) } else { format!("  ← {}", c) })
        } else if !s.is_empty() {
            gray(format!("  ← {}", s))
        } else {
            gray("  ← 默认颜色".to_string())
        };
        println!("  {}  {}", preview, desc);
    }
    println!();
    println!("{}", bold_green("PATH 路径值规则（与 pp 一致）:"));
    for rule in path_rules::PATH_RULES {
        let sample = if rule.pattern.contains('*') {
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
        let (c, s) = (rule.color, rule.style);
        println!("  {}  {}", path_rules::styled(&sample, c, s), gray(&format!("  ← {}", rule.pattern)));
    }
    println!();
    println!("{}", gray("无颜色的变量保持默认终端颜色"));
    println!("{}", gray("无颜色匹配的路径保持默认终端颜色"));
}

fn match_var(name: &str) -> (&'static str, &'static str) {
    for rule in VAR_RULES {
        if rule.name.eq_ignore_ascii_case(name) {
            return (rule.color, rule.style);
        }
    }
    ("", "")
}
