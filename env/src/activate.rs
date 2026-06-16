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
    /// 别名列表
    #[serde(default)]
    pub aliases: Vec<String>,
}

impl Default for TagDef {
    fn default() -> Self {
        Self {
            path: Vec::new(),
            var: HashMap::new(),
            prompt: String::new(),
            aliases: Vec::new(),
        }
    }
}

// ── 路径 ──────────────────────────────────

/// tags 目录：%LOCALAPPDATA%\e\tags\
fn get_tags_dir() -> PathBuf {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(local).join("e").join("tags")
}

/// 公开访问 tags 目录路径
pub fn tags_dir() -> PathBuf {
    get_tags_dir()
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

    // 检查名称是否已被其他 tag 作为别名占用
    if let Some((canonical, _)) = resolve_tag(name) {
        return Err(format!("'{}' 已被 tag '{}' 的别名占用", name, canonical));
    }

    let def = TagDef::default();
    save_tag_at_path(&path, &def)?;
    Ok(path)
}

/// 保存 TagDef 到指定路径
fn save_tag_at_path(path: &std::path::Path, def: &TagDef) -> Result<(), String> {
    let yaml = serde_yaml::to_string(def).map_err(|e| format!("序列化失败: {}", e))?;
    std::fs::write(path, &yaml).map_err(|e| format!("写入失败: {}", e))?;
    Ok(())
}

/// 保存 TagDef（根据名称定位文件）
fn save_tag(name: &str, def: &TagDef) -> Result<(), String> {
    let (canonical, _) = resolve_tag(name).ok_or_else(|| format!("tag '{}' 不存在", name))?;
    save_tag_at_path(&tag_path(&canonical), def)
}

// ── 路径编辑 ──────────────────────────────────

/// 为 tag 添加一个 PATH 目录
pub fn tag_add_path(name: &str, path_to_add: &str) -> Result<(), String> {
    let (canonical, mut def) = resolve_tag(name).ok_or_else(|| format!("tag '{}' 不存在", name))?;
    def.path.push(path_to_add.to_string());
    save_tag(&canonical, &def)
}

/// 从 tag 移除一个 PATH 目录（按索引）
pub fn tag_remove_path(name: &str, index: usize) -> Result<(), String> {
    let (canonical, mut def) = resolve_tag(name).ok_or_else(|| format!("tag '{}' 不存在", name))?;
    if index >= def.path.len() {
        return Err(format!("索引越界: 共有 {} 条路径，索引 0..{}", def.path.len(), def.path.len().saturating_sub(1)));
    }
    let removed = def.path.remove(index);
    save_tag(&canonical, &def)?;
    println!("{} 已移除路径 [{}]: {}", green("✓"), index, removed);
    Ok(())
}

// ── 变量编辑 ──────────────────────────────────

/// 为 tag 设置一个环境变量（新增或修改）
pub fn tag_set_var(name: &str, key: &str, value: &str) -> Result<(), String> {
    let (canonical, mut def) = resolve_tag(name).ok_or_else(|| format!("tag '{}' 不存在", name))?;
    let old = def.var.insert(key.to_string(), value.to_string());
    save_tag(&canonical, &def)?;
    if old.is_some() {
        println!("{} 已修改变量 {}={} (原值: {})", green("✓"), key, value, old.unwrap());
    } else {
        println!("{} 已添加变量 {}={}", green("✓"), key, value);
    }
    Ok(())
}

/// 从 tag 移除一个环境变量
pub fn tag_remove_var(name: &str, key: &str) -> Result<(), String> {
    let (canonical, mut def) = resolve_tag(name).ok_or_else(|| format!("tag '{}' 不存在", name))?;
    if def.var.remove(key).is_some() {
        save_tag(&canonical, &def)?;
        println!("{} 已移除变量 {}", green("✓"), key);
        Ok(())
    } else {
        Err(format!("tag '{}' 中不存在变量 '{}'", canonical, key))
    }
}

// ── PROMPT 编辑 ──────────────────────────────────

/// 设置 tag 的 PROMPT
pub fn tag_set_prompt(name: &str, value: &str) -> Result<(), String> {
    let (canonical, mut def) = resolve_tag(name).ok_or_else(|| format!("tag '{}' 不存在", name))?;
    def.prompt = value.to_string();
    save_tag(&canonical, &def)?;
    println!("{} PROMPT 已设置为: {}", green("✓"), value);
    Ok(())
}

/// 清除 tag 的 PROMPT
pub fn tag_clear_prompt(name: &str) -> Result<(), String> {
    let (canonical, mut def) = resolve_tag(name).ok_or_else(|| format!("tag '{}' 不存在", name))?;
    def.prompt.clear();
    save_tag(&canonical, &def)?;
    println!("{} PROMPT 已清除", green("✓"));
    Ok(())
}

// ── 别名编辑 ──────────────────────────────────

/// 为 tag 添加别名
pub fn tag_add_alias(name: &str, alias: &str) -> Result<(), String> {
    let (canonical, mut def) = resolve_tag(name).ok_or_else(|| format!("tag '{}' 不存在", name))?;

    // 检查别名是否已被占用
    if let Some((existing, _)) = resolve_tag(alias) {
        if existing != canonical {
            return Err(format!("'{}' 已被 tag '{}' 作为别名或名称占用", alias, existing));
        }
    }

    if def.aliases.iter().any(|a| a.eq_ignore_ascii_case(alias)) {
        return Err(format!("别名 '{}' 已存在", alias));
    }

    def.aliases.push(alias.to_string());
    save_tag(&canonical, &def)?;
    println!("{} 已添加别名: {}", green("✓"), alias);
    Ok(())
}

/// 从 tag 移除别名
pub fn tag_remove_alias(name: &str, alias: &str) -> Result<(), String> {
    let (canonical, mut def) = resolve_tag(name).ok_or_else(|| format!("tag '{}' 不存在", name))?;
    let pos = def.aliases.iter().position(|a| a.eq_ignore_ascii_case(alias));
    match pos {
        Some(i) => {
            def.aliases.remove(i);
            save_tag(&canonical, &def)?;
            println!("{} 已移除别名: {}", green("✓"), alias);
            Ok(())
        }
        None => Err(format!("tag '{}' 中没有别名 '{}'", canonical, alias)),
    }
}

/// 删除 tag（支持别名）
pub fn remove_tag(name: &str) -> Result<(), String> {
    let path = tag_path(name);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("删除失败: {}", e))
    } else if let Some((canonical, _)) = resolve_tag(name) {
        let path = tag_path(&canonical);
        std::fs::remove_file(&path).map_err(|e| format!("删除失败: {}", e))
    } else {
        Err(format!("tag '{}' 不存在", name))
    }
}

/// 根据名称或别名查找 tag，返回 (规范名, 配置)
pub fn resolve_tag(query: &str) -> Option<(String, TagDef)> {
    // 先直接按文件名匹配
    if let Some(def) = load_tag(query) {
        return Some((query.to_string(), def));
    }
    // 再扫描所有 tag 匹配别名
    for name in list_tags() {
        if let Some(def) = load_tag(&name) {
            if def.aliases.iter().any(|a| a.eq_ignore_ascii_case(query)) {
                return Some((name, def));
            }
        }
    }
    None
}

// ── 脚本生成 ──────────────────────────────────

/// 组合多个 tag 生成 cmd 一行命令
pub fn generate_script(tags: &[String]) -> Result<String, String> {
    if tags.is_empty() {
        return Err("请指定至少一个 tag".into());
    }

    let mut defs = Vec::new();
    let mut resolved_names = Vec::new();
    for name in tags {
        let (canonical, def) = resolve_tag(name)
            .ok_or_else(|| format!("tag 或别名 '{}' 不存在", name))?;
        defs.push(def);
        resolved_names.push(canonical);
    }

    let tag_label = resolved_names.join("+");
    let mut parts: Vec<String> = Vec::new();

    // PATH 前置（后列出的 tag 优先）— 合并为一条 SET，避免 %PATH% 重复展开
    let mut all_paths: Vec<&str> = Vec::new();
    for def in defs.iter().rev() {
        for p in &def.path {
            all_paths.push(p);
        }
    }
    if !all_paths.is_empty() {
        let joined = all_paths.join(";");
        parts.push(format!("@SET \"PATH={};%PATH%\"", joined));
    }

    // 环境变量
    for def in &defs {
        for (k, v) in &def.var {
            parts.push(format!("@SET \"{}={}\"", k, v));
        }
    }

    // PROMPT = [tag1+tag2+...] $P$G
    parts.push(format!("@SET \"PROMPT=[{}] $P$G\"", tag_label));

    Ok(parts.join(" && "))
}

// ── 打印 ──────────────────────────────────

/// 渲染 tag 列表，自适应终端宽度，丰富颜色，别名列
pub fn print_tag_list() {
    let tags = list_tags();
    if tags.is_empty() {
        println!("  {} 在 {} 下没有 tag", gray("•"), gray(get_tags_dir().display().to_string()));
        println!("  {} 使用 e tag create <名称> 创建一个", gray("•"));
        println!();
        println!("  {}", gray(format!("目录: {}", get_tags_dir().display())));
        return;
    }

    // 收集所有 tag 数据
    struct TagRow {
        name: String,
        aliases: Vec<String>,
        alias_label: String,    // 逗号分隔的别名文本
        paths: Vec<String>,
        vars: Vec<(String, String)>,
    }

    let rows: Vec<TagRow> = tags.iter().filter_map(|name| {
        let def = load_tag(name)?;
        let mut vars: Vec<(String, String)> = def.var.into_iter().collect();
        vars.sort_by(|a, b| a.0.cmp(&b.0));
        let alias_label = def.aliases.join(", ");
        Some(TagRow {
            name: name.clone(),
            aliases: def.aliases,
            alias_label,
            paths: def.path,
            vars,
        })
    }).collect();

    let has_aliases = rows.iter().any(|r| !r.aliases.is_empty());

    // ── 计算自然宽度 ──

    let term_w = terminal_width();
    let indent = 2usize;
    let gap = 2usize;
    let indent_s = " ".repeat(indent);
    let sep = " ".repeat(gap);

    let name_nat = rows.iter().map(|r| r.name.display_width()).max().unwrap_or(6);
    let alias_nat = if has_aliases {
        rows.iter().map(|r| r.alias_label.display_width()).max().unwrap_or(0)
    } else {
        0
    };
    let path_nat = rows.iter().flat_map(|r| r.paths.iter()).map(|p| p.display_width()).max().unwrap_or(0);
    let var_nat = rows.iter().flat_map(|r| r.vars.iter()).map(|(k, v)| format!("{}={}", k, v).display_width()).max().unwrap_or(0);

    let col_count = if has_aliases { 4usize } else { 3usize };
    let reserved = indent + gap * (col_count - 1);

    // ── 自适应分配列宽 ──

    let (name_w, alias_w, path_w, var_w) = if has_aliases {
        let min_name = 6usize;
        let min_alias = 4usize;
        let min_path = 10usize;
        let min_var = 8usize;
        let total_nat = name_nat + alias_nat + path_nat + var_nat + reserved;

        if total_nat <= term_w {
            (name_nat.max(min_name), alias_nat.max(min_alias), path_nat.max(min_path), var_nat.max(min_var))
        } else {
            let available = term_w.saturating_sub(reserved);
            let nw = name_nat.min(8).max(min_name);
            let aw = alias_nat.min(10).max(min_alias);
            let remain = available.saturating_sub(nw + aw);
            // 路径:变量 = 6:4
            let pw = (remain as f64 * 0.6).floor() as usize;
            let vw = remain.saturating_sub(pw);
            (nw, aw, pw.max(min_path), vw.max(min_var))
        }
    } else {
        let min_name = 6usize;
        let min_path = 10usize;
        let min_var = 8usize;
        let total_nat = name_nat + path_nat + var_nat + reserved;

        if total_nat <= term_w {
            (name_nat.max(min_name), 0, path_nat.max(min_path), var_nat.max(min_var))
        } else {
            let available = term_w.saturating_sub(reserved);
            let nw = name_nat.min(8).max(min_name);
            let remain = available.saturating_sub(nw);
            let pw = (remain as f64 * 0.6).floor() as usize;
            let vw = remain.saturating_sub(pw);
            (nw, 0, pw.max(min_path), vw.max(min_var))
        }
    };

    // ── 表头 ──

    if has_aliases {
        let h_n = pad_left("名称", name_w);
        let h_a = pad_left("别名", alias_w);
        let h_p = pad_left("路径", path_w);
        let h_v = pad_left("变量", var_w);
        println!("{}{}{}{}{}{}{}{}",
            indent_s,
            bold_cyan(&h_n), &sep,
            bold_magenta(&h_a), &sep,
            bold_green(&h_p), &sep,
            bold_yellow(&h_v));

        let s_n = "-".repeat(name_w);
        let s_a = "-".repeat(alias_w);
        let s_p = "-".repeat(path_w);
        let s_v = "-".repeat(var_w);
        println!("{}{}{}{}{}{}{}{}",
            indent_s,
            gray(&s_n), &sep, gray(&s_a), &sep, gray(&s_p), &sep, gray(&s_v));
    } else {
        let h_n = pad_left("名称", name_w);
        let h_p = pad_left("路径", path_w);
        let h_v = pad_left("变量", var_w);
        println!("{}{}{}{}{}{}",
            indent_s,
            bold_cyan(&h_n), &sep,
            bold_green(&h_p), &sep,
            bold_yellow(&h_v));

        let s_n = "-".repeat(name_w);
        let s_p = "-".repeat(path_w);
        let s_v = "-".repeat(var_w);
        println!("{}{}{}{}{}{}",
            indent_s,
            gray(&s_n), &sep, gray(&s_p), &sep, gray(&s_v));
    }
    println!();

    // ── 数据行 ──

    for r in &rows {
        let max_lines = r.paths.len().max(r.vars.len()).max(r.aliases.len()).max(1);

        for i in 0..max_lines {
            // 名称列：首行显示
            let name_str = if i == 0 {
                cell_text(&r.name, name_w)
            } else {
                " ".repeat(name_w)
            };

            // 别名列
            let alias_str = if has_aliases {
                if i < r.aliases.len() {
                    cell_text(&r.aliases[i], alias_w)
                } else {
                    " ".repeat(alias_w)
                }
            } else {
                String::new()
            };

            // 路径列
            let path_str = if i < r.paths.len() {
                cell_text(&r.paths[i], path_w)
            } else {
                " ".repeat(path_w)
            };

            // 变量列
            let var_str = if i < r.vars.len() {
                let (k, v) = &r.vars[i];
                cell_text(&format!("{}={}", k, v), var_w)
            } else {
                " ".repeat(var_w)
            };

            if has_aliases {
                if i == 0 {
                    println!("{}{}  {}  {}  {}",
                        indent_s, cyan(&name_str),
                        magenta(&alias_str), green(&path_str), yellow(&var_str));
                } else {
                    println!("{}{}  {}  {}  {}",
                        indent_s, gray(&name_str),
                        bright_magenta(&alias_str), bright_green(&path_str), bright_yellow(&var_str));
                }
            } else {
                if i == 0 {
                    println!("{}{}  {}  {}",
                        indent_s, cyan(&name_str),
                        green(&path_str), yellow(&var_str));
                } else {
                    println!("{}{}  {}  {}",
                        indent_s, gray(&name_str),
                        bright_green(&path_str), bright_yellow(&var_str));
                }
            }
        }
    }

    println!();
    println!("  {}  {}", gray("组合:"), cyan("e gen <tag1> <tag2> ..."));
    println!("  {}  {}", gray("剪贴板:"), cyan("e gen --copy <tag1> <tag2> ..."));
    println!();
    println!("  {}", gray(format!("目录: {}", get_tags_dir().display())));
}

/// 单元格文本渲染：若内容能放入列宽则左对齐填充，否则截断加 ...
fn cell_text(text: &str, col_w: usize) -> String {
    let dw = text.display_width();
    if dw <= col_w {
        pad_left(text, col_w)
    } else if col_w <= 3 {
        text.to_string()
    } else {
        color::truncate(text, col_w)
    }
}

/// 获取终端宽度（Windows API）
pub fn terminal_width() -> usize {
    #[repr(C)]
    struct ConsoleScreenBufferInfo {
        dw_size: [u16; 2],
        dw_cursor: [u16; 2],
        w_attrs: u16,
        sr_window: [u16; 4],
        dw_max: [u16; 2],
    }

    unsafe extern "system" {
        fn GetStdHandle(id: u32) -> isize;
        fn GetConsoleScreenBufferInfo(h: isize, info: *mut ConsoleScreenBufferInfo) -> i32;
    }

    unsafe {
        let handle = GetStdHandle(0xFFFFFFF5u32); // STD_OUTPUT_HANDLE
        if handle == -1 || handle == 0 {
            return 120;
        }
        let mut info: ConsoleScreenBufferInfo = std::mem::zeroed();
        if GetConsoleScreenBufferInfo(handle, &mut info) != 0 {
            (info.sr_window[2] - info.sr_window[0] + 1).max(40) as usize
        } else {
            120
        }
    }
}

// ── 辅助工具 ─────────────────────────────────

/// 批处理转义：% → %%
#[allow(dead_code)]
fn esc_bat(s: &str) -> String {
    s.replace('%', "%%")
}

/// PowerShell 单引号转义：' → ''
#[allow(dead_code)]
fn esc_ps1(s: &str) -> String {
    s.replace('\'', "''")
}
