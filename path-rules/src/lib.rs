//! PATH 着色规则共享库 — pp 和 ss 共用

/// 着色规则：一条路径模式对应的颜色和样式。
#[derive(Debug, Clone)]
pub struct Rule {
    pub pattern: &'static str,
    pub color: &'static str,
    pub style: &'static str,
}

/// PATH 着色规则（源自原 pp.yaml 配置）
pub static PATH_RULES: &[Rule] = &[
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
    Rule { pattern: r"*\.cargo\bin", color: "lightpurple", style: "" },
    Rule { pattern: r"\\*", color: "", style: "underline" },
];

/// 匹配 PATH 路径，返回 (color_name, style_name)。
pub fn match_path(text: &str) -> (&'static str, &'static str) {
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

pub fn wildmatch(pattern: &str, text: &str) -> bool {
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

/// 用颜色名和样式名包装文本。
pub fn styled(text: &str, color_name: &str, style_name: &str) -> String {
    let code = match color_name.to_lowercase().as_str() {
        "black" => 30, "red" => 31, "green" => 32,
        "yellow" => 33, "blue" => 34, "magenta" | "purple" => 35,
        "cyan" => 36, "white" => 37, "gray" => 90,
        "lightred" => 91, "lightgreen" => 92, "lightyellow" => 93,
        "lightblue" => 94, "lightmagenta" | "lightpurple" => 95,
        "lightcyan" => 96, "brightwhite" => 97,
        _ => return text.to_string(),
    };
    let mut s = color::Style::new(code);
    match style_name.to_lowercase().as_str() {
        "bold" => s = s.bold(),
        "dim" => s = s.dim(),
        "italic" => s = s.italic(),
        "underline" => s = s.underline(),
        _ => {}
    }
    s.paint(text)
}

/// 打印 PATH 着色规则预览（供 `--style` 选项使用）。
pub fn print_path_styles() {
    use color::*;
    println!("{}", bold_cyan("PATH 着色规则预览"));
    println!("{}", gray("━".repeat(40)));
    println!();
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
                .replace(r"*\.cargo\bin", r"C:\Users\Me\.cargo\bin")
                .replace(r"\\*", r"\\server\share\folder")
        } else {
            rule.pattern.to_string()
        };
        let (c, s) = (rule.color, rule.style);
        println!("  {}  {}", styled(&sample, c, s), gray(&format!("  ← {}", rule.pattern)));
    }
    println!();
    println!("{}", gray("无颜色的路径保持默认终端颜色"));
}
