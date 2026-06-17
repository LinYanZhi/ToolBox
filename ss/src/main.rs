/// ss — 轻量环境变量显示工具，根据硬编码规则给变量着色

use std::process;
use std::collections::BTreeMap;

struct Rule {
    name: &'static str,
    color: &'static str,
    style: &'static str,
}

/// 环境变量着色规则（源自原 ss.yaml 配置）
static VAR_RULES: &[Rule] = &[
    // 灰色（系统级变量、内部变量）
    Rule { name: "SYSTEMDRIVE", color: "gray", style: "" },
    Rule { name: "SYSTEMROOT", color: "gray", style: "" },
    Rule { name: "TEMP", color: "gray", style: "" },
    Rule { name: "TMP", color: "gray", style: "" },
    Rule { name: "USERNAME", color: "gray", style: "" },
    Rule { name: "USERPROFILE", color: "gray", style: "" },
    Rule { name: "COMSPEC", color: "gray", style: "" },
    Rule { name: "PROMPT", color: "gray", style: "" },
    // 绿色
    Rule { name: "CONDA_HOME", color: "green", style: "" },
    // 浅蓝
    Rule { name: "MYSQL_HOME", color: "lightblue", style: "" },
    // 浅青
    Rule { name: "NVM_HOME", color: "lightcyan", style: "" },
    Rule { name: "NVM_SYMLINK", color: "lightcyan", style: "" },
    // 紫色
    Rule { name: "ENV", color: "purple", style: "" },
    Rule { name: "ENV_PATH", color: "purple", style: "" },
    Rule { name: "KMP_DUPLICATE_LIB_OK", color: "purple", style: "" },
    // 红色
    Rule { name: "REDIS_HOME", color: "red", style: "" },
    // 浅黄
    Rule { name: "PYTHON_HOME", color: "lightyellow", style: "" },
    // 蓝色
    Rule { name: "JAVA_HOME", color: "blue", style: "" },
    Rule { name: "GOPATH", color: "blue", style: "" },
    // 浅红
    Rule { name: "SHADOWBOT_CULTURE", color: "lightred", style: "" },
    Rule { name: "SHADOWBOT_ROOT_X64", color: "lightred", style: "" },
    // 浅紫
    Rule { name: "RUSTUP_UPDATE_ROOT", color: "lightpurple", style: "" },
    Rule { name: "RUSTUP_DIST_SERVER", color: "lightpurple", style: "" },
    // 变量样式规则
    Rule { name: "WT_SESSION", color: "", style: "italic" },
];

/// 需要排除（不显示）的变量
static EXCLUDE_NAMES: &[&str] = &[
    // PyInstaller 内部变量
    "_PYI_APPLICATION_HOME_DIR",
    "_PYI_ARCHIVE_FILE",
    "_PYI_PARENT_PROCESS_LEVEL",
    // Python 虚拟环境旧变量
    "_OLD_VIRTUAL_PATH",
    "_OLD_VIRTUAL_PROMPT",
    // 自定义环境变量
    "_MY_CURRENT_ENV",
    "_MY_ENV_ACTIVATED",
    "_MY_OLD_PATH",
    "_MY_OLD_PROMPT",
    "_MY_FORCE",
];

fn main() {
    color::enable_ansi();

    let args: Vec<String> = std::env::args().collect();
    let no_color = args.iter().any(|a| a == "-n" || a == "--no-color");
    let left_align = args.iter().any(|a| a == "-l" || a == "--left");

    // ss gui — 打开环境变量对话框
    if args.get(1).map(|s| s.as_str()) == Some("gui") {
        let _ = process::Command::new("rundll32.exe")
            .args(["sysdm.cpl,EditEnvironmentVariables"])
            .spawn();
        return;
    }

    // 收集所有环境变量
    let mut vars: BTreeMap<String, String> = BTreeMap::new();
    for (k, v) in std::env::vars() {
        if k.starts_with('=') {
            continue;
        }
        if is_excluded(&k) {
            continue;
        }
        vars.insert(k, v);
    }

    if vars.is_empty() {
        return;
    }

    let max_len = vars.keys().map(|k| k.len()).max().unwrap_or(0);

    println!();
    for (name, value) in &vars {
        if no_color {
            if left_align {
                println!("{:<width$} = {value}", name, width = max_len);
            } else {
                println!("{:>width$} = {value}", name, width = max_len);
            }
        } else {
            let (color, style) = match_var(name);
            let padding = if left_align {
                " ".repeat(max_len - name.len())
            } else {
                " ".repeat(max_len - name.len())
            };
            if left_align {
                print!("{}{} = ", styled(name, color, style), padding);
            } else {
                print!("{}{} = ", padding, styled(name, color, style));
            }
            println!("{}", styled(value, color, style));
        }
    }
}

/// 判断变量是否排除
fn is_excluded(name: &str) -> bool {
    let lower = name.to_lowercase();
    EXCLUDE_NAMES.iter().any(|e| e.to_lowercase() == lower)
}

/// 匹配变量名，返回 (color_name, style_name)
fn match_var(name: &str) -> (&'static str, &'static str) {
    for rule in VAR_RULES {
        if rule.name.eq_ignore_ascii_case(name) {
            return (rule.color, rule.style);
        }
    }
    ("", "")
}

/// 颜色名称 → ANSI 前景色码
fn color_code(name: &str) -> Option<u8> {
    match name.to_lowercase().as_str() {
        "black" => Some(30),
        "red" => Some(31),
        "green" => Some(32),
        "yellow" => Some(33),
        "blue" => Some(34),
        "magenta" | "purple" => Some(35),
        "cyan" => Some(36),
        "white" => Some(37),
        "gray" => Some(90),
        "lightred" => Some(91),
        "lightgreen" => Some(92),
        "lightyellow" => Some(93),
        "lightblue" => Some(94),
        "lightmagenta" | "lightpurple" => Some(95),
        "lightcyan" => Some(96),
        "brightwhite" => Some(97),
        _ => None,
    }
}

/// 样式名称 → ANSI 样式码
fn style_code(name: &str) -> Option<u8> {
    match name.to_lowercase().as_str() {
        "bold" => Some(1),
        "dim" => Some(2),
        "italic" => Some(3),
        "underline" => Some(4),
        "blink" => Some(5),
        "reverse" => Some(7),
        "hidden" => Some(8),
        "strikethrough" => Some(9),
        _ => None,
    }
}

/// 给文本加上 ANSI 样式
fn styled(text: &str, color: &str, style: &str) -> String {
    let mut codes: Vec<u8> = Vec::new();
    if let Some(c) = color_code(color) {
        codes.push(c);
    }
    if let Some(s) = style_code(style) {
        codes.push(s);
    }
    if codes.is_empty() {
        return text.to_string();
    }
    let code_str = codes
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(";");
    format!("\x1b[{code_str}m{text}\x1b[0m")
}
