use std::collections::HashMap;
use std::fs;

use anyhow::Context;
use serde::{Deserialize, Serialize};

// ── 内嵌的默认软件源 ──────────────────────────────────

/// 编译时嵌入的默认 source.json 内容。
const EMBEDDED_JSON: &str = include_str!("../source.json");

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

// ── 内部数据结构 ──────────────────────────────────────

/// 软件源数据库文件（software.json）的结构。
#[derive(Debug, Deserialize, Serialize)]
struct SoftwareDatabase {
    /// 格式版本号，用于未来迁移。
    version: u32,
    /// 内置软件（编译时嵌入，升级时自动合并）。
    builtin: HashMap<String, SoftwareDef>,
    /// 用户通过 `as add` 添加的本地扩展。
    local: HashMap<String, SoftwareDef>,
}

// ── 初始化 ────────────────────────────────────────────

/// 确保 software.json 文件存在（首次运行写入默认值）。
pub fn ensure_initialized() -> anyhow::Result<()> {
    let path = crate::paths::software_json_path();
    if path.is_file() {
        return Ok(());
    }
    // 创建父目录
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, EMBEDDED_JSON)?;
    Ok(())
}

// ── 内部读写 ──────────────────────────────────────────

/// 从磁盘读取 software.json，返回合并后的 full（builtin + local）字典。
fn read_full_db() -> anyhow::Result<HashMap<String, SoftwareDef>> {
    let path = crate::paths::software_json_path();
    // 如果文件不存在，先初始化
    if !path.is_file() {
        ensure_initialized()?;
    }
    let data = fs::read_to_string(&path)?;
    let db: SoftwareDatabase = serde_json::from_str(&data)
        .with_context(|| format!("解析 {} 失败", path.display()))?;

    let mut all = db.builtin;
    // local 覆盖 builtin（同名时 local 优先）
    for (name, def) in db.local {
        all.insert(name, def);
    }
    Ok(all)
}

// ── 公开 API ──────────────────────────────────────────

/// 在所有软件定义（builtin + local）中查找指定软件。
pub fn read_software_def(name: &str) -> anyhow::Result<SoftwareDef> {
    let lower = name.to_lowercase();
    let all = read_full_db()?;

    // 1. name 精确匹配
    if let Some(sd) = all.get(&lower) {
        return Ok(sd.clone());
    }

    // 2. display_name / aliases 匹配
    for sd in all.values() {
        if sd.display_name.to_lowercase() == lower {
            return Ok(sd.clone());
        }
        if sd.aliases.iter().any(|a| a.to_lowercase() == lower) {
            return Ok(sd.clone());
        }
        // 3. name 部分匹配（例如传入 "as" 匹配 SoftwareDef{name:"as"}）
        if sd.name.to_lowercase() == lower {
            return Ok(sd.clone());
        }
    }

    anyhow::bail!("未找到软件 '{}' 的定义", name)
}

/// 列出所有软件定义（builtin + local）。
pub fn list_software_defs() -> anyhow::Result<Vec<SoftwareDef>> {
    let all = read_full_db()?;
    let mut defs: Vec<SoftwareDef> = all.into_values().collect();
    defs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(defs)
}

/// 读取自研工具定义（从统一的 software.json 中筛选 kind="self" 的条目）。
pub fn read_tool_def(name: &str) -> anyhow::Result<SoftwareDef> {
    let sd = read_software_def(name)?;
    if sd.kind == "self" {
        Ok(sd)
    } else {
        anyhow::bail!("'{}' 不是自研工具", name)
    }
}

/// 列出所有自研工具定义。
pub fn list_tool_defs() -> anyhow::Result<Vec<SoftwareDef>> {
    let all = read_full_db()?;
    let mut defs: Vec<SoftwareDef> = all.into_values()
        .filter(|sd| sd.kind == "self")
        .collect();
    defs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(defs)
}

#[allow(dead_code)]
/// 添加/更新本地扩展软件定义。
/// 写入到 software.json 的 local 段，同名会覆盖。
pub fn add_local_software(def: SoftwareDef) -> anyhow::Result<()> {
    let path = crate::paths::software_json_path();
    if !path.is_file() {
        ensure_initialized()?;
    }
    let data = fs::read_to_string(&path)?;
    let mut db: SoftwareDatabase = serde_json::from_str(&data)
        .with_context(|| format!("解析 {} 失败", path.display()))?;

    db.local.insert(def.name.clone(), def);

    let json = serde_json::to_string_pretty(&db)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

#[allow(dead_code)]
/// 移除本地扩展软件定义。
pub fn remove_local_software(name: &str) -> anyhow::Result<()> {
    let path = crate::paths::software_json_path();
    if !path.is_file() {
        return Ok(());
    }
    let data = fs::read_to_string(&path)?;
    let mut db: SoftwareDatabase = serde_json::from_str(&data)
        .with_context(|| format!("解析 {} 失败", path.display()))?;

    db.local.remove(name);

    let json = serde_json::to_string_pretty(&db)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

// ── Installation records ─────────────────────────────────

pub fn read_installed_db() -> anyhow::Result<HashMap<String, InstallRecord>> {
    let path = crate::paths::installed_json();
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let data = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data).unwrap_or_default())
}

pub fn write_installed_db(db: &HashMap<String, InstallRecord>) -> anyhow::Result<()> {
    let dir = crate::paths::apps_dir();
    fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(db)?;
    let target = crate::paths::installed_json();
    let tmp = target.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &target)?;
    Ok(())
}

pub fn record_installation(
    name: &str,
    version: &str,
    install_path: &str,
    version_provenance: &str,
    source_version: &str,
    installer_type: &str,
    file_sha256: &str,
) -> anyhow::Result<()> {
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

/// 返回软件源的最后更新时间（文件修改时间）。
pub fn read_source_updated() -> String {
    let path = crate::paths::software_json_path();
    if !path.is_file() {
        return String::new();
    }
    match path.metadata()
        .and_then(|m| m.modified())
        .map(|t| {
            let secs = t.duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            chrono_like(secs)
        }) {
        Ok(s) => s,
        Err(_) => String::new(),
    }
}

/// 简单的 unix timestamp → YYYY-MM-DD 格式化（不依赖 chrono crate）。
fn chrono_like(secs: u64) -> String {
    // 使用 time::OffsetDateTime 
    let days = secs / 86400;
    let remaining = secs % 86400;
    let h = remaining / 3600;
    let m = (remaining % 3600) / 60;

    // 近似年份计算
    let mut y = 1970i64;
    let mut d = days as i64;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if d < days_in_year {
            break;
        }
        d -= days_in_year;
        y += 1;
    }
    let is_leap_yr = is_leap(y);
    let month_days = [31, if is_leap_yr { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut mo = 1u32;
    for &md in &month_days {
        if d < md {
            break;
        }
        d -= md;
        mo += 1;
    }
    let day = d + 1;
    format!("{:04}-{:02}-{:02} {:02}:{:02}", y, mo, day, h, m)
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// 工具源的最后更新（与主源共用同一文件）。
pub fn read_tool_source_updated() -> String {
    read_source_updated()
}

/// 总是返回 true（内置源始终可用）。
pub fn has_any_source() -> bool {
    true
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
        "yellow"      => color::yellow(text),
        "cyan"        => color::cyan(text),
        "white"       => color::white(text),
        "gray"        => color::gray(text),
        "bright-green"   => color::bold_green(text),
        "bright-blue"    => color::bold_blue(text),
        "bright-red"     => color::bold_red(text),
        "bright-yellow"  => color::bold_yellow(text),
        "bright-cyan"    => color::bold_cyan(text),
        "bright-magenta" => color::bold_magenta(text),
        _ => color::cyan(text),
    }
}
