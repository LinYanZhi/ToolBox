use std::collections::HashMap;
use std::path::PathBuf;

use color::*;
use serde::{Deserialize, Serialize};

// ── Tag 定义 ──────────────────────────────────

/// 一个 tag 表示一组环境配置（PATH 目录 + 环境变量 + PROMPT）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagDef {
    /// 前置追加到 PATH 的目录列表（顺序敏感）
    #[serde(default)]
    pub path: Vec<String>,
    /// 设置的环境变量
    #[serde(default)]
    pub var: HashMap<String, String>,
    /// PROMPT 覆盖（可选）
    #[serde(default)]
    pub prompt: String,
}

impl Default for TagDef {
    fn default() -> Self {
        Self {
            path: Vec::new(),
            var: HashMap::new(),
            prompt: String::new(),
        }
    }
}

// ── 路径 ──────────────────────────────────

/// tags 目录：%LOCALAPPDATA%\e\tags\
fn get_tags_dir() -> PathBuf {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(local).join("e").join("tags")
}

fn tag_path(name: &str) -> PathBuf {
    get_tags_dir().join(format!("{}.yaml", name))
}

// ── CRUD ──────────────────────────────────

/// 列举所有 tag
pub fn list_tags() -> Vec<String> {
    let dir = get_tags_dir();
    if !dir.exists() {
        return Vec::new();
    }
    let mut tags = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().map_or(false, |e| e == "yaml") {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    if !stem.starts_with('.') {
                        tags.push(stem.to_string());
                    }
                }
            }
        }
    }
    tags.sort();
    tags
}

/// 加载一个 tag
pub fn load_tag(name: &str) -> Option<TagDef> {
    let content = std::fs::read_to_string(tag_path(name)).ok()?;
    serde_yaml::from_str(&content).ok()
}

/// 创建 tag（默认空配置）
pub fn create_tag(name: &str) -> Result<PathBuf, String> {
    let dir = get_tags_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("无法创建目录 {}: {}", dir.display(), e))?;

    let path = tag_path(name);
    if path.exists() {
        return Err(format!("tag '{}' 已存在", name));
    }

    let def = TagDef::default();
    let yaml = serde_yaml::to_string(&def).map_err(|e| format!("序列化失败: {}", e))?;
    std::fs::write(&path, &yaml).map_err(|e| format!("写入失败: {}", e))?;
    Ok(path)
}

/// 删除 tag
pub fn remove_tag(name: &str) -> Result<(), String> {
    let path = tag_path(name);
    if !path.exists() {
        return Err(format!("tag '{}' 不存在", name));
    }
    std::fs::remove_file(&path).map_err(|e| format!("删除失败: {}", e))
}

// ── 脚本生成 ──────────────────────────────────

/// 组合多个 tag 生成 cmd 脚本
pub fn generate_script(tags: &[String]) -> Result<String, String> {
    if tags.is_empty() {
        return Err("请指定至少一个 tag".into());
    }

    let mut defs = Vec::new();
    for name in tags {
        let def = load_tag(name).ok_or_else(|| format!("tag '{}' 不存在", name))?;
        defs.push(def);
    }

    let tag_label = tags.join("+");

    let mut lines: Vec<String> = Vec::new();
    lines.push("@ECHO OFF".into());
    lines.push(format!("REM e: 组合 tag: {}", tag_label));
    lines.push(String::new());

    // PATH 追加（后列出的 tag 优先，所以从后往前 prepend）
    for def in defs.iter().rev() {
        for p in &def.path {
            lines.push(format!("@SET \"PATH=%PATH%;{}\"", p));
        }
    }

    // 环境变量
    for def in &defs {
        for (k, v) in &def.var {
            lines.push(format!("@SET \"{}={}\"", k, v));
        }
    }

    // PROMPT = [tag1+tag2+...] $P$G
    lines.push(format!("@SET \"PROMPT=[{}] $P$G\"", tag_label));

    lines.push(String::new());
    lines.push("@ECHO 环境已就绪，请按任意键退出 . . .".into());
    lines.push("@PAUSE >NUL".into());

    Ok(lines.join("\r\n"))
}

// ── 打印 ──────────────────────────────────

pub fn print_tag_list() {
    let tags = list_tags();
    if tags.is_empty() {
        println!("  {} 在 {} 下没有 tag", gray("•"), gray(get_tags_dir().display().to_string()));
        println!("  {} 使用 e tag create <名称> 创建一个", gray("•"));
        println!();
        println!("  {}", gray(format!("目录: {}", get_tags_dir().display())));
        return;
    }

    let rows: Vec<(&str, usize, usize, Vec<String>)> = tags.iter().map(|name| {
        let def = load_tag(name);
        let paths = def.as_ref().map(|d| d.path.clone()).unwrap_or_default();
        let vars = def.as_ref().map(|d| d.var.keys().cloned().collect::<Vec<_>>()).unwrap_or_default();
        (name.as_str(), paths.len(), vars.len(), paths)
    }).collect();

    let max_name = rows.iter().map(|(n, _, _, _)| n.display_width()).max().unwrap_or(6);
    let max_path_cnt = rows.iter().map(|(_, c, _, _)| c).max().unwrap_or(&1);
    let path_w = format!("PATH:{}", max_path_cnt).len().max("PATH".len());
    let max_var_cnt = rows.iter().map(|(_, _, c, _)| c).max().unwrap_or(&1);
    let var_w = format!("变量:{}", max_var_cnt).len().max("变量".len());

    // 收集所有路径用于计算最大宽度
    let max_path_display = rows.iter().flat_map(|(_, _, _, paths)| paths.iter()).map(|p| p.display_width()).max().unwrap_or(0);
    let preview_w = max_path_display.min(60);

    let hdr_name = pad_left("名称", max_name);
    let hdr_path = pad_left("PATH", path_w.max(4));
    let hdr_var = pad_left("变量", var_w.max(4));
    println!("  {}", bold_cyan(format!("{}  {}  {}  路径", hdr_name, hdr_path, hdr_var)));
    let sep_name = format!("{:─>width$}", "", width = max_name + 2);
    let sep_path = format!("{:─>width$}", "", width = path_w.max(4) + 1);
    let sep_var = format!("{:─>width$}", "", width = var_w.max(4) + 1);
    println!("  {}", gray(format!("{}  {}  {}  ────────────", sep_name, sep_path, sep_var)));
    println!();

    for (name, pc, vc, paths) in &rows {
        let path_str = if paths.is_empty() {
            String::new()
        } else {
            let joined = paths.join("; ");
            if joined.display_width() > preview_w {
                let mut s = String::new();
                let mut w = 0;
                for p in paths {
                    let add = if s.is_empty() { p.clone() } else { format!("; {}", p) };
                    let add_w = add.display_width();
                    if w + add_w > preview_w && !s.is_empty() { break; }
                    s.push_str(&add);
                    w += add_w;
                }
                s.push('…');
                s
            } else {
                joined
            }
        };

        println!("  {}  {}  {}  {}",
            pad_left(&cyan(name), max_name),
            pad_left(&format!("{}", pc), path_w),
            pad_left(&format!("{}", vc), var_w),
            gray(path_str));
    }
    println!();
    println!("  {}  {}", gray("组合:"), cyan("e gen <tag1> <tag2> ..."));
    println!("  {}  {}", gray("剪贴板:"), cyan("e gen --copy <tag1> <tag2> ..."));
    println!();
    println!("  {}", gray(format!("目录: {}", get_tags_dir().display())));
}
