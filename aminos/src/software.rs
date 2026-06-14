use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

use crate::paths;

// ── JSON Schema ──────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Detection {
    pub display_name: Option<String>,
    pub publisher: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VersionInfo {
    pub urls: Vec<String>,
    #[serde(default)]
    pub arch: String,
    #[serde(default)]
    pub installer_type: String,
    #[serde(default)]
    pub install_args: Vec<String>,
    #[serde(default)]
    pub uninstall_args: Vec<String>,
    #[serde(default)]
    pub detection: Option<Detection>,
    #[serde(default)]
    pub shortcut_candidates: Vec<String>,
    #[serde(default)]
    pub install_dir_candidates: Vec<String>,
    /// 便携版的入口可执行文件名（如 "Snipaste.exe", "7zFM.exe"）。
    /// 未设置时自动扫描目录中的 exe。
    #[serde(default)]
    pub entry_point: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SoftwareDef {
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub homepage: String,
    #[serde(default)]
    pub default_version: String,
    /// 软件类型：空/未设置 = 第三方软件, "self" = 自研工具
    #[serde(default)]
    pub kind: String,
    pub versions: HashMap<String, VersionInfo>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct InstallRecord {
    pub version: String,
    pub install_path: String,
    /// 版本来源：pe | source | registry
    #[serde(default = "default_provenance")]
    pub version_provenance: String,
    /// 源 JSON 中声明的版本 key
    #[serde(default)]
    pub source_version: String,
    /// 安装时间（Unix 时间戳）
    #[serde(default)]
    pub install_time: u64,
    /// 安装类型（如 "portable", "nsis", "inno" 等），升级时复用
    #[serde(default)]
    pub installer_type: String,
}

fn default_provenance() -> String {
    "source".to_string()
}

// ── Software definitions ─────────────────────────────────

/// 委托到 `config::source` 更新源定义。
pub fn update_sources() -> anyhow::Result<()> {
    let builtin: Vec<String> = vec![
        "https://ghproxy.net/https://raw.githubusercontent.com/LinYanZhi/aminos-source/main",
        "https://cdn.jsdelivr.net/gh/LinYanZhi/aminos-source@main",
        "https://raw.githubusercontent.com/LinYanZhi/aminos-source/main",
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect();

    let repo = config::SourceRepo::new(builtin);
    let result = config::source::update_sources(&paths::source_dir(), &repo);
    // 更新后清除缓存，下次 list 重新解析
    clear_defs_cache();
    result
}

/// Read a single software definition. Supports:
/// 1. Exact filename match
/// 2. Case-insensitive filename match
/// 3. Match by `display_name`
/// 4. Match by `aliases`
pub fn read_software_def(name: &str) -> anyhow::Result<SoftwareDef> {
    let source = paths::source_dir();
    let lower = name.to_lowercase();

    // 空源检查
    if !source.is_dir() || source.read_dir().map(|mut d| d.next().is_none()).unwrap_or(true) {
        anyhow::bail!("未找到源定义。请先运行: as env source update");
    }

    // 1. Exact match
    let exact = source.join(format!("{}.json", lower));
    if exact.exists() {
        return parse_json(&exact);
    }

    // Collect all .json files and try matches
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&source) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                candidates.push(path);
            }
        }
    }

    // 2. Case-insensitive filename match
    for p in &candidates {
        if p.file_stem()
            .and_then(|s| s.to_str())
            .map_or(false, |s| s.to_lowercase() == lower)
        {
            return parse_json(p);
        }
    }

    // 3-4. display_name / aliases match
    for p in &candidates {
        if let Ok(sd) = parse_json(p) {
            if sd.display_name.to_lowercase() == lower {
                return Ok(sd);
            }
            if sd.aliases.iter().any(|a| a.to_lowercase() == lower) {
                return Ok(sd);
            }
        }
    }

    bail!("未找到软件 '{}' 的定义", name)
}

/// 缓存, 避免多次重复解析所有 JSON 文件。
/// 在 `update_sources()` 中自动失效。
static DEFS_CACHE: std::sync::Mutex<Option<Vec<SoftwareDef>>> = std::sync::Mutex::new(None);

/// 清除源定义缓存（由 source update 时调用）。
pub fn clear_defs_cache() {
    if let Ok(mut cache) = DEFS_CACHE.lock() {
        *cache = None;
    }
}

/// List all available software definitions.
pub fn list_software_defs() -> anyhow::Result<Vec<SoftwareDef>> {
    // 命中缓存则直接返回
    if let Ok(cache) = DEFS_CACHE.lock() {
        if let Some(ref defs) = *cache {
            return Ok((*defs).clone());
        }
    }

    let source = paths::source_dir();
    if !source.is_dir() {
        return Ok(Vec::new());
    }

    let mut defs: Vec<SoftwareDef> = Vec::new();
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&source) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().map_or(false, |e| e == "json") && p.file_name().and_then(|n| n.to_str()) != Some("index.json") {
                paths.push(p);
            }
        }
    }
    paths.sort();

    for p in paths {
        if let Ok(sd) = parse_json(&p) {
            defs.push(sd);
        }
    }

    // 写入缓存
    if let Ok(mut cache) = DEFS_CACHE.lock() {
        *cache = Some(defs.clone());
    }

    Ok(defs)
}

// ── Installation records ─────────────────────────────────

pub fn read_installed_db() -> anyhow::Result<HashMap<String, InstallRecord>> {
    let path = paths::installed_json();
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let data = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data).unwrap_or_default())
}

pub fn write_installed_db(db: &HashMap<String, InstallRecord>) -> anyhow::Result<()> {
    let dir = paths::apps_dir();
    fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(db)?;

    // 原子写入：先写临时文件，再 rename，防止崩溃导致 installed.json 截断
    let target = paths::installed_json();
    let tmp = target.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &target)?;
    Ok(())
}

pub fn record_installation(name: &str, version: &str, install_path: &str, version_provenance: &str, source_version: &str, installer_type: &str) -> anyhow::Result<()> {
    let mut db = read_installed_db()?;
    db.insert(
        name.to_string(),
        InstallRecord {
            version: version.to_string(),
            install_path: install_path.to_string(),
            version_provenance: version_provenance.to_string(),
            source_version: source_version.to_string(),
            install_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            installer_type: installer_type.to_string(),
        },
    );
    write_installed_db(&db)
}

pub fn remove_installation_record(name: &str) -> anyhow::Result<()> {
    let mut db = read_installed_db()?;
    db.remove(name);
    write_installed_db(&db)
}

// ── Helpers ──────────────────────────────────────────────

fn parse_json(path: &PathBuf) -> anyhow::Result<SoftwareDef> {
    let data = fs::read_to_string(path)
        .with_context(|| format!("读取文件失败: {}", path.display()))?;
    serde_json::from_str(&data)
        .with_context(|| format!("解析 JSON 失败: {}", path.display()))
}
