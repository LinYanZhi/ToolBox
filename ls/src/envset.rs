use std::collections::HashMap;

use crate::config;

/// 颜色代码 → ANSI 数字
const ANSI_COLORS: &[(&str, &str)] = &[
    ("black", "30"), ("gray", "90"),
    ("blue", "34"), ("lightblue", "94"),
    ("green", "32"), ("lightgreen", "92"),
    ("cyan", "36"), ("lightcyan", "96"),
    ("red", "31"), ("lightred", "91"),
    ("purple", "35"), ("lightpurple", "95"),
    ("yellow", "33"), ("lightyellow", "93"),
    ("white", "37"), ("brightwhite", "97"),
    ("bold", "1"), ("dim", "2"), ("italic", "3"), ("underline", "4"),
    ("blink", "5"), ("reverse", "7"), ("hidden", "8"), ("strikethrough", "9"),
];

fn color_name_to_ansi(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    for (n, code) in ANSI_COLORS {
        if *n == lower {
            return Some(code);
        }
    }
    None
}

/// 用 ANSI 颜色代码包裹文本
fn paint(text: &str, color_or_style: &str) -> String {
    if let Some(code) = color_name_to_ansi(color_or_style) {
        format!("\x1b[{}m{}\x1b[0m", code, text)
    } else {
        // 可能是数字颜色代码
        format!("\x1b[{}m{}\x1b[0m", color_or_style, text)
    }
}

/// 显示所有环境变量（对应 --set）
pub fn show_env_vars(left_align: bool, no_color: bool) {
    let color_map = config::get_variable_color_map();
    let exclude_list = config::get_exclude_set();
    let mut env_vars: Vec<(String, String)> = std::env::vars().collect();

    // 过滤排除项
    env_vars.retain(|(name, _)| {
        !exclude_list.iter().any(|e| e.eq_ignore_ascii_case(name))
    });

    if env_vars.is_empty() {
        return;
    }

    let max_name_len = env_vars.iter().map(|(n, _)| n.len()).max().unwrap_or(0);

    // 排序
    env_vars.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    println!();
    for (name, value) in &env_vars {
        let var_name = if left_align {
            format!("{:<width$}", name, width = max_name_len)
        } else {
            format!("{:>width$}", name, width = max_name_len)
        };

        if no_color {
            println!("{} = {}", var_name, value);
        } else if let Some(color) = color_map.get(name) {
            let painted_name = paint(name, color);
            // 对齐：先计算颜色的对齐
            let name_colored = if left_align {
                format!("{:<width$}", painted_name, width = max_name_len)
            } else {
                format!("{:>width$}", painted_name, width = max_name_len)
            };
            println!("{} = {}", name_colored, value);
        } else {
            println!("{} = {}", var_name, value);
        }
    }
}

/// 显示 PATH 环境变量（对应 --path）
pub fn show_path(no_color: bool) {
    let color_map = config::get_path_color_map();
    let path = std::env::var("PATH").unwrap_or_default();
    let paths: Vec<&str> = path.split(';').collect();

    println!();
    for p in &paths {
        if p.is_empty() {
            continue;
        }
        if no_color {
            println!("{}", p);
        } else if let Some(color) = get_matching_color(p, &color_map) {
            println!("{}", paint(p, &color));
        } else {
            println!("{}", p);
        }
    }
}

/// 查找路径匹配的颜色/样式
fn get_matching_color(path: &str, color_map: &HashMap<String, String>) -> Option<String> {
    for (pattern, color) in color_map {
        if config::wildmatch(pattern, path) {
            return Some(color.clone());
        }
    }
    None
}
