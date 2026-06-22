use std::collections::HashMap;
use std::fs;
use std::io::Write;

use anyhow::Context;
use color::DisplayWidth;
use serde::{Deserialize, Serialize};

/// 注册表检测配置
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DetectConfig {
    /// 注册表中的 DisplayName（用于匹配）
    pub display_name: String,
    /// 可选发布者（提高匹配精度）
    #[serde(default)]
    pub publisher: Option<String>,
}

/// 单个软件条目
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SoftwareEntry {
    #[serde(default)]
    pub aliases: Vec<String>,
    /// 分类（如：工具、开发工具、浏览器...）
    #[serde(default)]
    pub category: Option<String>,
    /// 注册表检测配置（可选，有则 as 能识别已安装的旧版）
    #[serde(default)]
    pub detect: Option<DetectConfig>,
    pub versions: HashMap<String, VersionEntry>,
}

/// 单个版本的配置
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct VersionEntry {
    /// 注册表中实际记录的版本号（与 source.json 的版本 key 可能不同）
    #[serde(default)]
    pub registry_version: Option<String>,
    /// url → type（installer / portable）
    pub urls: HashMap<String, Vec<String>>,
}

/// 安装记录
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InstallRecord {
    pub version: String,
    /// installer / portable
    pub r#type: String,
    pub install_path: String,
    pub install_time: String,
}

/// 安装记录数据库
#[derive(Debug, Deserialize, Serialize)]
pub struct InstalledDatabase {
    #[serde(flatten)]
    pub records: HashMap<String, InstallRecord>,
}

// ── 编译时嵌入的默认源 ────────────────────────────

const EMBEDDED_JSON: &str = include_str!("../source.json");

/// 读取所有软件源条目（仅从嵌入的编译时数据读取，不访问外部文件）
pub fn read_all_entries() -> anyhow::Result<HashMap<String, SoftwareEntry>> {
    let v: serde_json::Value = serde_json::from_str(EMBEDDED_JSON)
        .context("解析嵌入的 source.json 失败")?;
    let builtin = v.get("builtin")
        .context("source.json 中缺少 builtin 字段")?;
    let entries: HashMap<String, SoftwareEntry> = serde_json::from_value(builtin.clone())
        .context("解析 builtin 条目失败")?;
    Ok(entries)
}

// ── 软件查找 ──────────────────────────────────────

/// 查找软件，如果找不到或有多匹配，交互式询问用户选择。
/// context 是操作说明（如"安装"、"卸载"），用于提示文本。
pub fn resolve_software(query: &str, context: &str) -> anyhow::Result<(String, SoftwareEntry)> {
    let all = read_all_entries()?;
    let lower = query.to_lowercase();

    // 1. 精确匹配 name
    if let Some(entry) = all.get(&lower) {
        return Ok((lower, entry.clone()));
    }

    // 2. aliases 精确匹配
    let mut matches: Vec<(String, SoftwareEntry)> = Vec::new();
    for (key, entry) in &all {
        if entry.aliases.iter().any(|a| a.to_lowercase() == lower) {
            matches.push((key.clone(), entry.clone()));
        }
    }

    if matches.len() == 1 {
        return Ok((matches[0].0.clone(), matches[0].1.clone()));
    }

    if matches.len() > 1 {
        println!("警告: '{}' 匹配到多个软件：", color::yellow(query));
        for (i, (name, _)) in matches.iter().enumerate() {
            println!("    {}. {}", i + 1, color::cyan(name));
        }
        print!("  请输入编号（1-{}，0=取消{}）: ", matches.len(), context);
        std::io::stdout().flush().ok();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        match input.trim().parse::<usize>() {
            Ok(n) if n >= 1 && n <= matches.len() => {
                return Ok((matches[n - 1].0.clone(), matches[n - 1].1.clone()));
            }
            _ => anyhow::bail!("已取消 {}", context),
        }
    }

    // 3. 无匹配 → 收集建议
    let suggestions = suggest_similar(query, &all);
    if suggestions.is_empty() {
        anyhow::bail!("未找到软件 '{}'，请检查名称", query);
    }

    let q_len = query.len();
    println!("警告: 未找到软件 '{}'", color::yellow(query));

    // 预计算行数据，确定各列宽度
    struct SuggRow {
        idx: String,
        name: String,
        pct: String,
        cat: String,
        alias: String,
    }
    let mut rows: Vec<SuggRow> = Vec::new();
    for (i, (score, sn)) in suggestions.iter().enumerate() {
        let entry = &all[sn];
        let pct = format!("{}%", (score * 100 / (10 + q_len * 2)).min(100));
        let cat = entry.category.as_deref().unwrap_or("").to_string();
        let alias = entry.aliases.join(", ");
        rows.push(SuggRow {
            idx: format!("{}.", i + 1),
            name: sn.clone(),
            pct,
            cat,
            alias,
        });
    }

    // 计算列宽（基于纯文本长度）
    let w_idx  = rows.iter().map(|r| r.idx.display_width()).max().unwrap_or(2);
    let w_name = rows.iter().map(|r| r.name.display_width()).max().unwrap_or(10);
    let w_pct  = rows.iter().map(|r| r.pct.display_width()).max().unwrap_or(4);
    let w_cat  = rows.iter().map(|r| r.cat.display_width()).max().unwrap_or(4);

    for r in &rows {
        println!(
            "    {} {} {} {} {}",
            pad_r(&r.idx, w_idx),
            color::cyan(&pad(&r.name, w_name)),
            color::gray(&pad(&r.pct, w_pct)),
            color::gray(&pad(&r.cat, w_cat)),
            color::gray(&r.alias),
        );
    }
    print!("  请输入编号（1-{}，0=取消{}）: ", suggestions.len(), context);
    std::io::stdout().flush().ok();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    match input.trim().parse::<usize>() {
        Ok(n) if n >= 1 && n <= suggestions.len() => {
            let name = &suggestions[n - 1].1;
            let entry = all[name].clone();
            Ok((name.clone(), entry))
        }
        _ => anyhow::bail!("已取消 {}", context),
    }
}

/// 收集与输入相似的软件名称（返回 (分数, 名称) 列表）。
pub(crate) fn suggest_similar(query: &str, all: &HashMap<String, SoftwareEntry>) -> Vec<(usize, String)> {
    let q = query.to_lowercase();
    let mut candidates: Vec<(usize, String)> = Vec::new();

    for (name, entry) in all {
        let n_lower = name.to_lowercase();
        if n_lower == q {
            continue;
        }

        let mut matched = false;

        // 检查 name
        let score = similarity_score(&q, &n_lower);
        if score > 0 {
            candidates.push((score, name.clone()));
            matched = true;
        }

        // 检查 aliases
        if !matched {
            for alias in &entry.aliases {
                let a_lower = alias.to_lowercase();
                if a_lower == q {
                    continue;
                }
                let score = similarity_score(&q, &a_lower);
                if score > 0 {
                    candidates.push((score, name.clone()));
                    break;
                }
            }
        }
    }

    // 按分数降序，取前 5（直接返回 (score, name) 元组）
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    candidates.truncate(5);
    candidates
}

/// 计算两个字符串的相似度分数（越高越相似）。
fn similarity_score(query: &str, target: &str) -> usize {
    let q = query.to_lowercase();
    let t = target.to_lowercase();

    // 1. 完全包含查询 → 高分
    if t.contains(&q) {
        return 10 + q.len() * 2; // 越长查询越精确
    }
    // 2. 查询包含目标的前缀 → 中分
    if q.contains(&t) {
        return 5 + t.len();
    }
    // 3. 公共前缀长度
    let common_prefix = q.chars().zip(t.chars()).take_while(|(a, b)| a == b).count();
    if common_prefix >= 2 {
        return common_prefix;
    }
    // 4. 公共子串（长度 >= 3，避免太短造成无意义匹配）
    if q.len() >= 3 && t.len() >= 3 {
        for w in (3..=q.len()).rev() {
            for i in 0..=q.len() - w {
                let sub = &q[i..i + w];
                if t.contains(sub) {
                    return w;
                }
            }
        }
    }

    0
}

// ── 安装记录读写 ──────────────────────────────────

pub fn read_installed() -> anyhow::Result<HashMap<String, InstallRecord>> {
    let path = crate::paths::installed_json();
    if !path.is_file() {
        return Ok(HashMap::new());
    }
    let data = fs::read_to_string(&path)?;
    let mut db: InstalledDatabase = serde_json::from_str(&data)
        .unwrap_or(InstalledDatabase { records: HashMap::new() });

    // 迁移：移除所有安装版记录（安装版信息在注册表中，不需要 as 维护）
    let had_installer = db.records.values().any(|r| r.r#type == "installer");
    db.records.retain(|_, r| r.r#type != "installer");
    if had_installer {
        let _ = write_installed(&db.records);
    }

    Ok(db.records)
}

pub fn write_installed(records: &HashMap<String, InstallRecord>) -> anyhow::Result<()> {
    let path = crate::paths::installed_json();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let db = InstalledDatabase {
        records: records.clone(),
    };
    let json = serde_json::to_string_pretty(&db)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

pub fn record_install(name: &str, ver: &str, r#type: &str, install_path: &str) -> anyhow::Result<()> {
    // 安装版不需要记录，注册表已有足够信息
    if r#type == "installer" {
        return Ok(());
    }
    let mut records = read_installed()?;
    records.insert(name.to_string(), InstallRecord {
        version: ver.to_string(),
        r#type: r#type.to_string(),
        install_path: install_path.to_string(),
        install_time: chrono_now(),
    });
    write_installed(&records)
}

pub fn remove_install_record(name: &str) -> anyhow::Result<()> {
    let mut records = read_installed()?;
    records.remove(name);
    write_installed(&records)
}

// ── 注册表检测 ────────────────────────────────────

/// 注册表检测到的已安装软件信息
#[derive(Debug, Clone)]
pub struct RegistryInfo {
    pub display_name: String,
    pub version: String,
    #[allow(dead_code)]
    pub publisher: Option<String>,
    #[allow(dead_code)]
    pub install_path: Option<String>,
    pub uninstall_string: Option<String>,
}

/// 扫描注册表，返回 (matched, unmatched)：
///   - matched: 能与 source entry 匹配的注册表条目（每个软件可能对应多个版本）
///   - unmatched: 匹配不到任何 source 的注册表条目
pub fn scan_registry_installed(
    entries: &HashMap<String, SoftwareEntry>,
) -> (HashMap<String, Vec<RegistryInfo>>, Vec<RegistryInfo>) {
    let all_reg = sys::registry::scan_all_installed();
    let mut matched: HashMap<String, Vec<RegistryInfo>> = HashMap::new();
    let mut unmatched = Vec::new();

    'next_reg: for reg in &all_reg {
        let dn = match reg.get("display_name") {
            Some(v) => v,
            None => continue,
        };
        let publisher = reg.get("publisher").map(|s| s.as_str());

        // 尝试匹配 source entry
        for (key, entry) in entries {
            if let Some(ref detect) = entry.detect {
                let dn_lower = dn.to_lowercase();
                let det_lower = detect.display_name.to_lowercase();
                let dn_matches = dn_lower.contains(&det_lower);

                // publisher 匹配：都提供时才检查，一方没有则忽略 publisher 条件
                let pub_matches = match (publisher, &detect.publisher) {
                    (Some(reg_pub), Some(det_pub)) => reg_pub.to_lowercase().contains(&det_pub.to_lowercase()),
                    _ => true,
                };
                if dn_matches && pub_matches {
                    matched.entry(key.clone()).or_default().push(RegistryInfo {
                        display_name: dn.clone(),
                        version: reg.get("version").cloned().unwrap_or_default(),
                        publisher: reg.get("publisher").cloned(),
                        install_path: reg.get("install_path").cloned(),
                        uninstall_string: reg.get("uninstall_string").cloned(),
                    });
                    continue 'next_reg;
                }
            }
        }

        // 没匹配到任何 source
        unmatched.push(RegistryInfo {
            display_name: dn.clone(),
            version: reg.get("version").cloned().unwrap_or_default(),
            publisher: reg.get("publisher").cloned(),
            install_path: reg.get("install_path").cloned(),
            uninstall_string: reg.get("uninstall_string").cloned(),
        });
    }

    (matched, unmatched)
}

/// 直接用 DetectConfig 从注册表查找已安装信息（无需完整 SoftwareEntry）
pub fn detect_from_registry_raw(detect: &DetectConfig) -> Option<RegistryInfo> {
    let reg_result = sys::registry::detect_installed_by(&detect.display_name, detect.publisher.as_deref())?;
    Some(RegistryInfo {
        display_name: reg_result.get("DisplayName").cloned().unwrap_or_default(),
        version: reg_result.get("DisplayVersion").cloned().unwrap_or_default(),
        publisher: reg_result.get("Publisher").cloned(),
        install_path: reg_result.get("InstallLocation").cloned(),
        uninstall_string: reg_result.get("UninstallString").cloned(),
    })
}

/// 从注册表查找所有匹配的已安装版本（返回所有匹配项，按版本去重）。
///
/// 用于卸载等需要处理多版本的场景。
pub fn detect_all_from_registry(entry: &SoftwareEntry) -> Vec<RegistryInfo> {
    let detect = match entry.detect.as_ref() {
        Some(d) => d,
        None => return vec![],
    };
    let results = sys::registry::detect_all_installed_by(&detect.display_name, detect.publisher.as_deref());
    let mut seen_versions = std::collections::HashSet::new();
    results.into_iter().filter_map(|r| {
        let version = r.get("DisplayVersion").cloned().unwrap_or_default();
        if version.is_empty() || !seen_versions.insert(version.clone()) {
            return None; // 按版本去重（同一版本在不同 hive 出现时不重复）
        }
        Some(RegistryInfo {
            display_name: r.get("DisplayName").cloned().unwrap_or_default(),
            version,
            publisher: r.get("Publisher").cloned(),
            install_path: r.get("InstallLocation").cloned(),
            uninstall_string: r.get("UninstallString").cloned(),
        })
    }).collect()
}

/// 当前时间 YYYY-MM-DD HH:MM:SS
pub fn now_str() -> String {
    chrono_now()
}

fn chrono_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format_unix(secs)
}

fn format_unix(secs: u64) -> String {
    let days = secs / 86400;
    let remaining = secs % 86400;
    let h = remaining / 3600;
    let m = (remaining % 3600) / 60;
    let s = remaining % 60;

    let mut y = 1970i64;
    let mut d = days as i64;
    loop {
        let diy = if is_leap(y) { 366 } else { 365 };
        if d < diy { break; }
        d -= diy;
        y += 1;
    }
    let leap = is_leap(y);
    let month_days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut mo = 1u32;
    for &md in &month_days {
        if d < md { break; }
        d -= md;
        mo += 1;
    }
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, d + 1, h, m, s)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// 左对齐填充到指定宽度（基于 CJK 感知宽度）
fn pad(s: &str, w: usize) -> String {
    let cw = s.display_width();
    if cw >= w { s.to_string() } else { format!("{}{}", s, " ".repeat(w - cw)) }
}

/// 右对齐填充到指定宽度（基于 CJK 感知宽度）
fn pad_r(s: &str, w: usize) -> String {
    let cw = s.display_width();
    if cw >= w { s.to_string() } else { format!("{}{}", " ".repeat(w - cw), s) }
}
