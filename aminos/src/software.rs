use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context};
use color;
use serde::{Deserialize, Serialize};
use crate::cmd_names;
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
    pub detection: Option<Detection>,
    #[serde(default)]
    pub shortcut_candidates: Vec<String>,
    #[serde(default)]
    pub install_dir_candidates: Vec<String>,
    /// 便携版的入口可执行文件名（如 "Snipaste.exe", "7zFM.exe"）。
    /// 未设置时自动扫描目录中的 exe。
    #[serde(default)]
    pub entry_point: Option<String>,
    /// 安装包的 SHA256 哈希（可选），用于自研工具检测内容变更。
    #[serde(default)]
    pub sha256: String,
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
    /// 软件类型：空/未设置 = 第三方软件, "self" = 自研工具
    #[serde(default)]
    pub kind: String,
    /// 首选下载后端（如 "ureq", "curl", "powershell", "rustrange"）。
    /// 设置后优先使用此后端下载，失败时回退到默认策略链。
    #[serde(default)]
    pub downloader: String,
    pub versions: HashMap<String, VersionInfo>,
}

impl SoftwareDef {
    /// 当 versions 只有一个版本时返回该版本 key，否则返回 None。
    pub fn single_version(&self) -> Option<&str> {
        if self.versions.len() == 1 {
            self.versions.keys().next().map(|s| s.as_str())
        } else {
            None
        }
    }

    /// 获取第一个版本 key（用于多版本软件的无默认场景）。
    pub fn first_version(&self) -> Option<&str> {
        self.versions.keys().next().map(|s| s.as_str())
    }
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
    /// 安装包/文件的 SHA256 哈希，用于检测内容变更。
    #[serde(default)]
    pub file_sha256: String,
}

fn default_provenance() -> String {
    "source".to_string()
}

// ── Software definitions ─────────────────────────────────

/// 委托到 `config::source` 更新源定义。
pub fn update_sources() -> anyhow::Result<()> {
    let builtin: Vec<String> = vec![
        format!("https://ghproxy.net/{}", crate::repo::SOURCE_RAW_URL),
        format!("https://cdn.jsdelivr.net/gh/{}@main", crate::repo::SOURCE_REPO),
        crate::repo::SOURCE_RAW_URL.to_string(),
    ];

    // 1. 同步所有分类源
    println!("{}", color::bold_cyan("同步软件源..."));
    for (i, cat) in paths::CATEGORIES.iter().enumerate() {
        let (_, label, _) = paths::CATEGORY_META[i];
        let urls: Vec<String> = builtin.iter().map(|u| format!("{}/apps/{}", u, cat)).collect();
        let repo = config::SourceRepo::new(urls);
        let dest = paths::category_dir(cat);
        if let Err(e) = config::source::update_sources(&dest, &repo) {
            eprintln!("  {} 同步 {} ({}) 失败: {}", color::red("错误"), label, cat, e);
        }
    }

    // 2. 同步自研工具源定义到 source/tools/
    println!("{}", color::bold_cyan("同步工具源..."));
    let tools_builtin: Vec<String> = builtin.iter().map(|u| format!("{}/tools", u)).collect();
    let tools_repo = config::SourceRepo::new(tools_builtin);
    config::source::update_sources(&paths::tools_source_dir(), &tools_repo)?;

    // 3. 同步第三方社区源
    let config_dir = paths::config_dir();
    let source_cfg = config::SourceConfig::new(config_dir);
    let entries = source_cfg.load();
    for entry in &entries {
        if !entry.enabled {
            continue;
        }
        println!("  {} {}", color::gray("同步源:"), color::cyan(&entry.name));
        let dest = paths::community_source_named(&entry.name);
        let repo = config::SourceRepo::new(vec![entry.url.clone()]);
        if let Err(e) = config::source::update_sources(&dest, &repo) {
            eprintln!("  {} 更新源 '{}' 失败: {}", color::red("错误"), entry.name, e);
        }
    }

    clear_defs_cache();
    clear_tool_cache();
    Ok(())
}

/// 在所有分类目录中查找软件定义。
pub fn read_software_def(name: &str) -> anyhow::Result<SoftwareDef> {
    let lower = name.to_lowercase();

    // 遍历所有分类目录
    for cat_dir in paths::app_category_dirs() {
        if !cat_dir.is_dir() {
            continue;
        }

        // 1. Exact match filename
        let exact = cat_dir.join(format!("{}.json", lower));
        if exact.exists() {
            return parse_json(&exact);
        }

        // 2. 文件名大小写不敏感
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Ok(entries) = fs::read_dir(&cat_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "json") && path.file_name().and_then(|n| n.to_str()) != Some("index.json") {
                    candidates.push(path);
                }
            }
        }
        for p in &candidates {
            if p.file_stem().and_then(|s| s.to_str()).map_or(false, |s| s.to_lowercase() == lower) {
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
    }

    // 5. 社区源中查找
    if let Ok(sd) = find_in_community_sources(name) {
        return Ok(sd);
    }

    bail!("未找到软件 '{}' 的定义。请先运行: {}", name, cmd_names::SOURCE_UPDATE_HINT)
}

/// 在第三方社区源中查找软件定义。
fn find_in_community_sources(name: &str) -> anyhow::Result<SoftwareDef> {
    let lower = name.to_lowercase();
    let config_dir = paths::config_dir();
    let source_cfg = config::SourceConfig::new(config_dir);
    let entries = source_cfg.load();

    for entry in &entries {
        if !entry.enabled {
            continue;
        }
        let dir = paths::community_source_named(&entry.name);
        if !dir.is_dir() {
            continue;
        }

        // Exact match
        let exact = dir.join(format!("{}.json", lower));
        if exact.exists() {
            if let Ok(sd) = parse_json(&exact) {
                return Ok(sd);
            }
        }

        // display_name / aliases match
        if let Ok(entries) = fs::read_dir(&dir) {
            for e in entries.flatten() {
                let path = e.path();
                if path.extension().map_or(false, |ext| ext == "json") && path.file_name().and_then(|n| n.to_str()) != Some("index.json") {
                    if let Ok(sd) = parse_json(&path) {
                        if sd.name.to_lowercase() == lower
                            || sd.display_name.to_lowercase() == lower
                            || sd.aliases.iter().any(|a| a.to_lowercase() == lower)
                        {
                            return Ok(sd);
                        }
                    }
                }
            }
        }
    }

    bail!("未在社区源中找到 '{}'", name)
}

/// 读取自研工具定义（从 source/tools/ 查找）
pub fn read_tool_def(name: &str) -> anyhow::Result<SoftwareDef> {
    let source = paths::tools_source_dir();
    let lower = name.to_lowercase();

    if !source.is_dir() || source.read_dir().map(|mut d| d.next().is_none()).unwrap_or(true) {
        anyhow::bail!("未找到工具源定义。请先运行: {}", cmd_names::SOURCE_UPDATE_HINT);
    }

    let exact = source.join(format!("{}.json", lower));
    if exact.exists() {
        return parse_json(&exact);
    }

    if let Ok(entries) = fs::read_dir(&source) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if stem.to_lowercase() == lower {
                    return parse_json(&path);
                }
            }
        }
    }

    bail!("未找到自研工具 '{}' 的定义", name)
}

/// 缓存, 避免多次重复解析所有 JSON 文件。
/// 在 `update_sources()` 中自动失效。
static DEFS_CACHE: std::sync::Mutex<Option<Vec<SoftwareDef>>> = std::sync::Mutex::new(None);
static TOOL_CACHE: std::sync::Mutex<Option<Vec<SoftwareDef>>> = std::sync::Mutex::new(None);

/// 清除源定义缓存（由 source update 时调用）。
pub fn clear_defs_cache() {
    if let Ok(mut cache) = DEFS_CACHE.lock() {
        *cache = None;
    }
}

/// 清除工具定义缓存
pub fn clear_tool_cache() {
    if let Ok(mut cache) = TOOL_CACHE.lock() {
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

    let mut defs: Vec<SoftwareDef> = Vec::new();

    // 遍历所有分类目录
    for cat_dir in paths::app_category_dirs() {
        if cat_dir.is_dir() {
            read_defs_from_dir(&cat_dir, &mut defs);
        }
    }

    // 合并社区源
    let mut seen: std::collections::HashSet<String> = defs.iter().map(|d| d.name.clone()).collect();
    let config_dir = paths::config_dir();
    let source_cfg = config::SourceConfig::new(config_dir);
    for entry in source_cfg.load() {
        if !entry.enabled {
            continue;
        }
        let comm_dir = paths::community_source_named(&entry.name);
        if !comm_dir.is_dir() {
            continue;
        }
        let mut community_defs = Vec::new();
        read_defs_from_dir(&comm_dir, &mut community_defs);
        for d in community_defs {
            if seen.insert(d.name.clone()) {
                defs.push(d);
            }
        }
    }

    // 写入缓存
    if let Ok(mut cache) = DEFS_CACHE.lock() {
        *cache = Some(defs.clone());
    }

    Ok(defs)
}

/// 从目录读取所有 JSON 定义到集合
fn read_defs_from_dir(dir: &std::path::Path, defs: &mut Vec<SoftwareDef>) {
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
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
}

/// 列出所有自研工具定义（从 source/tools/ 读取）
pub fn list_tool_defs() -> anyhow::Result<Vec<SoftwareDef>> {
    if let Ok(cache) = TOOL_CACHE.lock() {
        if let Some(ref defs) = *cache {
            return Ok((*defs).clone());
        }
    }

    let source = paths::tools_source_dir();
    if !source.is_dir() {
        return Ok(Vec::new());
    }

    let mut defs = Vec::new();
    let mut json_paths: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&source) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().map_or(false, |e| e == "json") && p.file_name().and_then(|n| n.to_str()) != Some("index.json") {
                json_paths.push(p);
            }
        }
    }
    json_paths.sort();

    for p in json_paths {
        if let Ok(sd) = parse_json(&p) {
            defs.push(sd);
        }
    }

    if let Ok(mut cache) = TOOL_CACHE.lock() {
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

pub fn record_installation(name: &str, version: &str, install_path: &str, version_provenance: &str, source_version: &str, installer_type: &str, file_sha256: &str) -> anyhow::Result<()> {
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
            file_sha256: file_sha256.to_string(),
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

/// 检查是否有任何缓存的源定义文件（至少一个分类目录非空）。
pub fn has_any_source() -> bool {
    paths::app_category_dirs().iter().any(|dir| {
        if !dir.is_dir() {
            return false;
        }
        dir.read_dir()
            .map(|mut entries| entries.any(|e| {
                e.ok().and_then(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    Some(name.ends_with(".json") && name != "index.json")
                }).unwrap_or(false)
            }))
            .unwrap_or(false)
    })
}

/// 读取源索引中的 `updated` 字段（源最后更新时间）。
pub fn read_source_updated() -> String {
    read_index_updated(&paths::apps_source_dir().join("index.json"))
}

/// 读取工具源索引中的 `updated` 字段。
pub fn read_tool_source_updated() -> String {
    read_index_updated(&paths::tools_source_dir().join("index.json"))
}

/// 从指定 index.json 中读取 `updated` 字段。
fn read_index_updated(index_path: &std::path::Path) -> String {
    if !index_path.is_file() {
        return String::new();
    }
    #[derive(serde::Deserialize)]
    struct IndexMeta {
        #[serde(default)]
        updated: String,
    }
    match std::fs::read_to_string(index_path)
        .and_then(|s| Ok(serde_json::from_str::<IndexMeta>(&s).map(|m| m.updated).unwrap_or_default()))
    {
        Ok(u) => u,
        Err(_) => String::new(),
    }
}

pub fn parse_json(path: &PathBuf) -> anyhow::Result<SoftwareDef> {
    let data = fs::read_to_string(path)
        .with_context(|| format!("读取文件失败: {}", path.display()))?;
    serde_json::from_str(&data)
        .with_context(|| format!("解析 JSON 失败: {}", path.display()))
}

/// 根据软件定义中的 color 字段将文本着色。
/// 优先使用代码内置的软件色彩映射表，未命中时使用 cyan 兜底。
pub fn paint_software(text: &str, def: &SoftwareDef) -> String {
    let color_name = software_color_map(&def.name)
        .unwrap_or("cyan");
    paint_by_color_name(text, color_name)
}

/// 软件→颜色的内置映射表（与 JSON 解耦，统一管理）。
fn software_color_map(name: &str) -> Option<&'static str> {
    Some(match name {
        // ── 编辑器 ──
        "vscode"         => "bright-blue",
        "trae"           => "bright-green",
        "cursor"         => "white",
        "notepadpp"      => "bright-green",
        // ── 浏览器 ──
        "chrome"         => "white",
        "edge"           => "bright-green",
        "firefox"        => "bright-yellow",
        // ── IDE ──
        "pycharm"        => "yellow",
        "datagrip"       => "bright-blue",
        "visual-studio"  => "bright-magenta",
        // ── 开发工具 ──
        "git"            => "yellow",
        "yingdao"        => "bright-red",
        "nvm"            => "green",
        "python"         => "yellow",
        "vmware"         => "bright-yellow",
        // ── 影音 ──
        "potplayer"      => "bright-yellow",
        "obs"            => "white",
        "steam"          => "blue",
        "netease-music"  => "bright-red",
        // ── 办公软件 ──
        "wps"            => "bright-red",
        "pdfgear"        => "red",
        "microsoft-office" => "white",
        // ── 社交聊天 ──
        "dingtalk"       => "blue",
        "wechat"         => "green",
        "wecom"          => "bright-blue",
        "feishu"         => "bright-blue",
        // ── 系统工具 ──
        "everything"     => "bright-yellow",
        "spacesniffer"   => "bright-yellow",
        "iobitunlocker"  => "yellow",
        "wepe"           => "bright-blue",
        // ── 效率增强 ──
        "7zip"             => "gray",
        "baidunetdisk"     => "bright-red",
        "pixpin"           => "bright-blue",
        "quarkclouddrive"  => "bright-blue",
        "snipaste"         => "bright-yellow",
        "ttime"            => "bright-green",
        "utools"           => "gray",
        "xunlei"           => "bright-blue",
        // ── 远程控制 ──
        "sunlogin"       => "bright-red",
        "todesk"         => "bright-blue",
        "uuremote"       => "cyan",
        // ── 安全防护 ──
        "huorong"        => "yellow",
        "watt-toolkit"   => "bright-blue",
        // ── 网络工具 ──
        "clash-verge"    => "bright-magenta",
        "flclash"        => "bright-blue",
        // ── 自研工具（全部灰色） ──
        "as"             => "gray",
        "ls"             => "gray",
        "lsd"            => "gray",
        "eza"            => "gray",
        "pp"             => "gray",
        "ss"             => "gray",
        "uv"             => "gray",
        "aria2c"         => "gray",
        _ => return None,
    })
}

/// 根据颜色名字符串将文本着色。
pub fn paint_by_color_name(text: &str, color_name: &str) -> String {
    match color_name {
        "green"       => color::green(text),
        "blue"        => color::blue(text),
        "red"         => color::red(text),
        "cyan"        => color::cyan(text),
        "yellow"      => color::yellow(text),
        "magenta"     => color::magenta(text),
        "gray"        => color::gray(text),
        "white"       => color::white(text),
        "black"       => color::black(text),
        "bold-green"  => color::bold_green(text),
        "bold-blue"   => color::bold_blue(text),
        "bold-cyan"   => color::bold_cyan(text),
        "bold-red"    => color::bold_red(text),
        "bold-yellow" => color::bold_yellow(text),
        "bright-green"   => color::bright_green(text),
        "bright-blue"    => color::bright_blue(text),
        "bright-cyan"    => color::bright_cyan(text),
        "bright-red"     => color::bright_red(text),
        "bright-yellow"  => color::bright_yellow(text),
        "bright-magenta" => color::bright_magenta(text),
        _ => color::cyan(text), // 默认兜底
    }
}
