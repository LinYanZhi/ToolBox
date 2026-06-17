/// pp — 轻量 PATH 显示工具，根据硬编码规则给路径着色

use std::process;

struct Rule {
    pattern: &'static str,
    color: &'static str,
    style: &'static str,
}

/// PATH 着色规则（源自原 pp.yaml 配置）
static PATH_RULES: &[Rule] = &[
    // 系统路径
    Rule { pattern: r"C:\Windows*", color: "gray", style: "" },
    // 用户脚本
    Rule { pattern: r"*\Notepad_file\*", color: "lightgreen", style: "" },
    // Git 环境
    Rule { pattern: r"*\Program Files\Git\cmd", color: "yellow", style: "" },
    Rule { pattern: r"*\AppData\Local\Microsoft\WindowsApps", color: "yellow", style: "" },
    // Python 环境
    Rule { pattern: r"*\Scripts", color: "lightyellow", style: "" },
    Rule { pattern: r"*\AppData\Local\Programs\Python\*", color: "lightyellow", style: "" },
    // Conda 环境
    Rule { pattern: r"*\Anaconda3\envs\*", color: "green", style: "" },
    Rule { pattern: r"*\Anaconda3\condabin", color: "green", style: "" },
    Rule { pattern: r"*\miniconda3\condabin", color: "green", style: "" },
    // Nvm 环境
    Rule { pattern: r"*\AppData\Roaming\nvm", color: "lightcyan", style: "" },
    Rule { pattern: r"*\AppData\Local\nvm", color: "lightcyan", style: "" },
    // Node 环境
    Rule { pattern: r"C:\Program Files\nodejs", color: "cyan", style: "" },
    Rule { pattern: r"*\nvm_nodejs", color: "cyan", style: "" },
    Rule { pattern: r"*\nvm\*", color: "cyan", style: "" },
    // MySQL
    Rule { pattern: r"*\mysql-8.0.35-winx64\bin", color: "lightblue", style: "" },
    // 影刀
    Rule { pattern: r"*\Program Files\ShadowBot", color: "lightred", style: "" },
    // 网络路径（下划线）
    Rule { pattern: r"\\*", color: "", style: "underline" },
];

fn main() {
    color::enable_ansi();

    let args: Vec<String> = std::env::args().collect();
    let no_color = args.iter().any(|a| a == "-n" || a == "--no-color");

    let path = match std::env::var("PATH") {
        Ok(p) => p,
        Err(_) => {
            eprintln!("错误: 无法读取 PATH 环境变量");
            process::exit(1);
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

/// 匹配路径，返回 (color_name, style_name)
fn match_path(text: &str) -> (&'static str, &'static str) {
    // 先精确匹配（没有 * 的规则优先）
    for rule in PATH_RULES {
        if !rule.pattern.contains('*') && rule.pattern.eq_ignore_ascii_case(text) {
            return (rule.color, rule.style);
        }
    }
    // 再通配符匹配
    for rule in PATH_RULES {
        if rule.pattern.contains('*') && wildmatch(rule.pattern, text) {
            return (rule.color, rule.style);
        }
    }
    ("", "")
}

/// 简单通配符匹配（仅支持 *）
fn wildmatch(pattern: &str, text: &str) -> bool {
    let p = pattern.to_lowercase();
    let t = text.to_lowercase();

    let parts: Vec<&str> = p.split('*').collect();
    if parts.is_empty() || (parts.len() == 1 && parts[0].is_empty()) {
        return true;
    }

    // 第一段必须匹配开头
    if !parts[0].is_empty() && !t.starts_with(parts[0]) {
        return false;
    }
    // 最后一段必须匹配结尾
    let last = parts.last().unwrap();
    if !last.is_empty() && !t.ends_with(last) {
        return false;
    }
    // 中间段按顺序出现
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
