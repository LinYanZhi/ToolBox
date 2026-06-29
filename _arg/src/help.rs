//! 动态中文帮助生成
//!
//! 规范：
//!   行 1  — 浅蓝色 exe 路径（左侧无空格）
//!   行 2  — 空行
//!   行 3  — "{name}是一个{about}"
//!   子标题 — 浅蓝色，无左空格
//!   短参数 — 浅绿色
//!   长参数 — 浅青色
//!   描述   — 灰色
//!   中文对齐

use crate::arg::{ArgDef, ArgKind};
use crate::cmd::Cmd;
use color::{self, DisplayWidth, Style};

/// 生成并打印帮助文本
pub fn print_help(cmd: &Cmd, exe_path: &str) {
    // ── 行1：exe 路径（浅蓝，无空格） ──
    println!("{}", bright_blue(exe_path));
    println!();

    // ── 行3：说明 ──
    println!("{}是一个{}", bright_cyan(&cmd.name), gray(&cmd.about));
    println!();

    // ── 用法 ──
    let mut parts = vec![cmd.name.clone()];
    if !cmd.args.is_empty() {
        parts.push("[选项]".to_string());
    }
    if !cmd.subs.is_empty() {
        parts.push("<子命令>".to_string());
    } else {
        for a in &cmd.args {
            if a.positional {
                parts.push(format!("<{}>", a.long));
            }
        }
    }
    let usage_line = parts.join(" ");

    // 收集所有待对齐的行：(左侧标签, 描述文字)
    let mut lines: Vec<(String, String)> = Vec::new();

    // 子命令
    if !cmd.subs.is_empty() {
        lines.push(("子命令:".to_string(), String::new()));
        for sub in &cmd.subs {
            let label = sub_label(&sub.cmd.name, &sub.aliases);
            let desc = if sub.cmd.about.is_empty() { String::new() } else { sub.cmd.about.clone() };
            lines.push((label, desc));
        }
    }

    // 选项
    if !cmd.args.is_empty() {
        lines.push(("选项:".to_string(), String::new()));
        for a in &cmd.args {
            let label = arg_short_label(a);
            let desc = a.desc.clone();
            lines.push((label, desc));
        }
    }

    // ── 对齐并输出 ──
    // 标题行不算宽度（因为标题独立显示）
    let normal_lines: Vec<_> = lines.iter().enumerate().filter(|(_, (l, _))| !l.ends_with(':')).collect();
    let max_label_w = normal_lines.iter()
        .map(|(_, (l, _))| l.display_width())
        .max()
        .unwrap_or(0);
    let label_col = max_label_w + 2; // 标签列宽（含间距）

    for (label, desc) in &lines {
        if label.ends_with(':') {
            // 子标题：浅蓝色，左侧无空格
            println!("{}", bright_blue(label));
        } else {
            // 普通行：2空格缩进 + 标签着色 + 对齐
            let colored = paint_label(label, cmd);
            let padding = label_col.saturating_sub(label.display_width());
            let pad = " ".repeat(padding);
            println!("  {}{}{}", colored, pad, gray(desc));
        }
    }

    // ── 用法（底部） ──
    if !usage_line.is_empty() {
        println!();
        println!("{} {}", gray("用法:"), bright_cyan(&usage_line));
    }
    println!();
}

/// 给参数标签上色（短名浅绿、长名浅青）
fn paint_label(label: &str, _cmd: &Cmd) -> String {
    // 解析 label，对短参数和长参数分别着色
    let mut result = String::new();
    let parts: Vec<&str> = label.split_whitespace().collect();

    for (i, part) in parts.iter().enumerate() {
        if i > 0 { result.push(' '); }
        if part.starts_with("--") {
            result.push_str(&bright_cyan(part));
        } else if part.starts_with('-') {
            result.push_str(&bright_green(part));
        } else {
            result.push_str(&bright_cyan(part));
        }
    }
    result
}

fn sub_label(name: &str, aliases: &[String]) -> String {
    let mut parts: Vec<String> = aliases.iter().map(|a| {
        if a.len() == 1 { format!("-{}", a) } else { format!("--{}", a) }
    }).collect();
    parts.insert(0, name.to_string());
    parts.join(", ")
}

fn arg_short_label(arg: &ArgDef) -> String {
    let mut parts = vec![];
    if let Some(short) = arg.short {
        parts.push(format!("-{}", short));
    }
    parts.push(format!("--{}", arg.long));
    if arg.kind == ArgKind::Value {
        let hint = if arg.optional {
            format!("[{}]", arg.long.to_uppercase())
        } else {
            format!("<{}>", arg.long.to_uppercase())
        };
        parts.push(hint);
    }
    parts.join(" ")
}

/// 打印版本信息
pub fn print_version(cmd: &Cmd, version: &str, extra: &str) {
    println!("{} {}", bright_cyan(&cmd.name), green(version));
    println!("{}", gray(&cmd.about));
    if !extra.is_empty() {
        println!("{}", gray(extra));
    }
}

/// 打印错误
pub fn print_error(msg: &str) {
    eprintln!("{} {}", red("错误:"), msg);
    eprintln!("{} 使用 --help 查看可用选项", gray("提示:"));
}

// ── 颜色快捷函数 ──

fn bright_blue(text: &str) -> String  { Style::new(94).paint(text) }
fn bright_cyan(text: &str) -> String  { Style::new(96).paint(text) }
fn bright_green(text: &str) -> String { Style::new(92).paint(text) }
fn gray(text: &str) -> String         { Style::new(90).paint(text) }
fn green(text: &str) -> String        { Style::new(32).paint(text) }
fn red(text: &str) -> String          { Style::new(31).paint(text) }
