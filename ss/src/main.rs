/// ss — 轻量环境变量显示工具，根据硬编码规则给变量着色

use std::collections::BTreeMap;

use arg::*;
use color;
use color::Style;

// ── CLI 定义 ────────────────────────────────────────

fn build_cmd() -> Cmd {
    Cmd::new("ss")
        .about("环境变量查看器，支持颜色区分、左对齐，支持打开系统环境变量对话框")
        .arg(flag("help", 'h', "显示帮助").global())
        .arg(flag("version", 'V', "显示版本号").global())
        .arg(flag("left", 'l', "左对齐变量名"))
        .arg(flag("no-color", 'n', "无颜色输出"))
        .arg(flag("style", 's', "显示着色规则预览"))
        .sub(
            Cmd::new("gui").about("打开系统环境变量对话框")
                .arg(flag("root", 'r', "以管理员权限打开（提权）"))
        )
        .sub(
            Cmd::new("path").about("显示 PATH 环境变量，按规则着色")
                .arg(flag("no-color", 'n', "无颜色输出"))
        )
}

// ── PATH 着色规则 ──

struct PathRule {
    pattern: &'static str,
    color: &'static str,
    style: &'static str,
}

static PATH_RULES: &[PathRule] = &[
    PathRule { pattern: r"C:\Windows*", color: "gray", style: "" },
    PathRule { pattern: r"*\Notepad_file\*", color: "lightgreen", style: "" },
    PathRule { pattern: r"*\Program Files\Git\cmd", color: "yellow", style: "" },
    PathRule { pattern: r"*\AppData\Local\Microsoft\WindowsApps", color: "yellow", style: "" },
    PathRule { pattern: r"*\Scripts", color: "lightyellow", style: "" },
    PathRule { pattern: r"*\AppData\Local\Programs\Python\*", color: "lightyellow", style: "" },
    PathRule { pattern: r"*\Anaconda3\envs\*", color: "green", style: "" },
    PathRule { pattern: r"*\Anaconda3\condabin", color: "green", style: "" },
    PathRule { pattern: r"*\miniconda3\condabin", color: "green", style: "" },
    PathRule { pattern: r"*\AppData\Roaming\nvm", color: "lightcyan", style: "" },
    PathRule { pattern: r"*\AppData\Local\nvm", color: "lightcyan", style: "" },
    PathRule { pattern: r"*\Program Files\nodejs", color: "cyan", style: "" },
    PathRule { pattern: r"*\nvm_nodejs", color: "cyan", style: "" },
    PathRule { pattern: r"*\nvm\*", color: "cyan", style: "" },
    PathRule { pattern: r"*\mysql*\bin", color: "lightblue", style: "" },
    PathRule { pattern: r"*\Program Files\ShadowBot", color: "lightred", style: "" },
    PathRule { pattern: r"*\.cargo\bin", color: "lightpurple", style: "" },
    PathRule { pattern: r"\\*", color: "", style: "underline" },
];

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
        if part.is_empty() { continue; }
        match t[pos..].find(part) {
            Some(idx) => pos += idx + part.len(),
            None => return false,
        }
    }
    true
}

fn path_styled(text: &str, color_name: &str, style_name: &str) -> String {
    let code = match color_name.to_lowercase().as_str() {
        "black" => 30, "red" => 31, "green" => 32,
        "yellow" => 33, "blue" => 34, "magenta" | "purple" => 35,
        "cyan" => 36, "white" => 37, "gray" => 90,
        "lightred" => 91, "lightgreen" => 92, "lightyellow" => 93,
        "lightblue" => 94, "lightmagenta" | "lightpurple" => 95,
        "lightcyan" => 96, "brightwhite" => 97,
        _ => return text.to_string(),
    };
    let mut s = Style::new(code);
    match style_name.to_lowercase().as_str() {
        "bold" => s = s.bold(),
        "dim" => s = s.dim(),
        "italic" => s = s.italic(),
        "underline" => s = s.underline(),
        _ => {}
    }
    s.paint(text)
}

struct Rule {
    name: &'static str,
    color: &'static str,
    style: &'static str,
}

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

static EXCLUDE_NAMES: &[&str] = &[
    "_PYI_APPLICATION_HOME_DIR", "_PYI_ARCHIVE_FILE", "_PYI_PARENT_PROCESS_LEVEL",
    "_OLD_VIRTUAL_PATH", "_OLD_VIRTUAL_PROMPT",
    "_MY_CURRENT_ENV", "_MY_ENV_ACTIVATED", "_MY_OLD_PATH", "_MY_OLD_PROMPT", "_MY_FORCE",
];

// ── main ────────────────────────────────────────────

fn main() {
    init();
    let cmd = build_cmd();

    let argv: Vec<String> = std::env::args().collect();
    let args = match parse(&cmd, &argv) {
        Ok(a) => a,
        Err(e) => {
            print_error(&e);
            return;
        }
    };

    let exe_path = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "ss".into());

    if args.flag("help") {
        print_help(&cmd, &exe_path);
        return;
    }
    if args.flag("version") {
        print_version(&cmd, "0.0.1", "");
        return;
    }
    if args.flag("style") {
        print_styles();
        return;
    }

    let no_color = args.flag("no-color");
    let left_align = args.flag("left");

    match args.sub.as_deref() {
        Some("gui") => {
            let sub = args.sub_args.as_ref().unwrap();
            if sub.flag("root") {
                let _ = std::process::Command::new("powershell")
                    .args([
                        "-NoProfile", "-Command",
                        "Start-Process -FilePath 'rundll32.exe' -ArgumentList 'sysdm.cpl,EditEnvironmentVariables' -Verb RunAs",
                    ]).spawn();
            } else {
                let _ = std::process::Command::new("rundll32.exe")
                    .args(["sysdm.cpl,EditEnvironmentVariables"])
                    .spawn();
            }
            return;
        }
        Some("path") => {
            let sub = args.sub_args.as_ref().unwrap();
            let path = match std::env::var("PATH") {
                Ok(p) => p,
                Err(_) => {
                    eprintln!("错误: 无法读取 PATH 环境变量");
                    std::process::exit(1);
                }
            };
            let nc = sub.flag("no-color");
            println!();
            for entry in path.split(';') {
                let entry = entry.trim();
                if entry.is_empty() { continue; }
                if nc {
                    println!("{entry}");
                } else {
                    let (c, s) = match_path(entry);
                    if c.is_empty() && s.is_empty() {
                        println!("{entry}");
                    } else {
                        println!("{}", path_styled(entry, c, s));
                    }
                }
            }
            return;
        }
        _ => {
            // 默认：显示所有环境变量
            print_env_vars(no_color, left_align);
        }
    }
}

fn print_env_vars(no_color: bool, left_align: bool) {
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
            let segments: Vec<&str> = value.split(';').collect();
            for (i, seg) in segments.iter().enumerate() {
                let seg = seg.trim();
                if seg.is_empty() { continue; }
                let (p_color, p_style) = match_path(seg);
                let colored = if p_color.is_empty() && p_style.is_empty() {
                    seg.to_string()
                } else {
                    path_styled(seg, p_color, p_style)
                };
                let line = if i == segments.len() - 1 || !value.contains(';') {
                    colored
                } else {
                    format!("{colored};")
                };
                if i == 0 {
                    println!("{} = {}", path_styled(&spaced, color, style), line);
                } else {
                    println!("{indent}{line}");
                }
            }
        } else if value.contains(';') {
            let segments: Vec<&str> = value.split(';').filter(|s| !s.is_empty()).collect();
            for (i, seg) in segments.iter().enumerate() {
                let line = if i == segments.len() - 1 { seg.to_string() } else { format!("{seg};") };
                if i == 0 {
                    if no_color {
                        println!("{spaced} = {line}");
                    } else {
                        println!("{} = {}", path_styled(&spaced, color, style), path_styled(&line, color, style));
                    }
                } else {
                    if no_color {
                        println!("{indent}{line}");
                    } else {
                        println!("{}{}", indent, path_styled(&line, color, style));
                    }
                }
            }
        } else {
            if no_color {
                println!("{spaced} = {value}");
            } else {
                println!("{} = {}", path_styled(&spaced, color, style), path_styled(value, color, style));
            }
        }
    }
}

fn is_excluded(name: &str) -> bool {
    let lower = name.to_lowercase();
    EXCLUDE_NAMES.iter().any(|e| e.to_lowercase() == lower)
}

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
            path_styled(&rule.name, c, s)
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
    println!("{}", bold_green("PATH 路径值规则:"));
    for rule in PATH_RULES {
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
        println!("  {}  {}", path_styled(&sample, c, s), gray(&format!("  ← {}", rule.pattern)));
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
