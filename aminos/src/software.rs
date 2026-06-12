use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

use crate::paths;

// ── JSON Schema ──────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
pub struct Detection {
    pub display_name: Option<String>,
    pub publisher: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
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
}

#[derive(Debug, Deserialize, Serialize)]
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
    pub versions: HashMap<String, VersionInfo>,
}

#[derive(Debug, Deserialize, Serialize)]
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
}

fn default_provenance() -> String {
    "source".to_string()
}

// ── Software definitions ─────────────────────────────────

/// Primary source repository (raw.githubusercontent.com).
const DEFAULT_SOURCE_REPO: &str = "https://raw.githubusercontent.com/LinYanZhi/aminos-source/main";

/// Fallback mirrors (tried in order when primary fails).
/// - ghproxy: GitHub proxy, goes through raw.githubusercontent.com (always latest)
/// - jsDelivr: GitHub CDN, slower to update but more stable
const FALLBACK_MIRRORS: &[&str] = &[
    "https://ghproxy.net/https://raw.githubusercontent.com/LinYanZhi/aminos-source/main",
    "https://cdn.jsdelivr.net/gh/LinYanZhi/aminos-source@main",
];

/// Update all source definitions from the remote repository.
/// Downloads index.json first, then each listed file.
/// No git required — plain HTTP. Tries primary repo then fallback mirrors.
pub fn update_sources() -> anyhow::Result<()> {
    let source_dir = paths::source_dir();
    fs::create_dir_all(&source_dir)?;

    let custom_repo = std::env::var("AMINOS_SOURCE_REPO").ok();

    // Build list of repos to try: custom > default > mirrors
    let repos: Vec<String> = if let Some(ref r) = custom_repo {
        vec![r.clone()]
    } else {
        let mut v = vec![DEFAULT_SOURCE_REPO.to_string()];
        v.extend(FALLBACK_MIRRORS.iter().map(|s| s.to_string()));
        v
    };

    // Cache-busting: append timestamp to bypass CDN cache
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // 1. Download index.json — try each repo in order
    let mut index_bytes = None;
    let mut used_repo = "";
    for repo in &repos {
        let cache_bust = if repo.contains("jsdelivr") { format!("?v={}", ts) } else { String::new() };
        let index_url = format!("{}/index.json{}", repo, cache_bust);
        print!("  尝试: {} ... ", index_url);
        match download_bytes(&index_url) {
            Ok(data) => {
                println!("OK");
                index_bytes = Some(data);
                used_repo = repo;
                break;
            }
            Err(e) => {
                println!("失败 ({})", e);
            }
        }
    }

    let index_bytes = index_bytes
        .context("所有镜像均无法连接，请检查网络。也可将 source/ 文件夹放到 as.exe 同级目录")?;

    #[derive(Deserialize)]
    struct Index {
        files: Vec<String>,
    }
    let index: Index = serde_json::from_slice(&index_bytes)
        .context("源索引格式错误")?;

    println!("  从 {} 下载，共 {} 个源文件", used_repo, index.files.len());

    // 2. Download each file
    let cache_bust = if used_repo.contains("jsdelivr") { format!("?v={}", ts) } else { String::new() };
    for fname in &index.files {
        let url = format!("{}/{}{}", used_repo, fname, cache_bust);
        let dest = source_dir.join(fname);
        print!("  {} ... ", fname);
        match download_bytes(&url) {
            Ok(data) => {
                fs::write(&dest, &data)?;
                println!("OK ({} B)", data.len());
            }
            Err(e) => {
                println!("跳过 ({})", e);
            }
        }
    }

    println!("\n  源更新完成。共 {} 个文件。", index.files.len());

    // 3. 清理本地多余文件（不在 index.json 中的旧文件）
    let index_set: std::collections::HashSet<&str> = index.files.iter().map(|s| s.as_str()).collect();
    if let Ok(entries) = fs::read_dir(&source_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name == "index.json" { continue; }
                    if !index_set.contains(name) {
                        let _ = fs::remove_file(&path);
                        println!("  清理旧文件: {}", name);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Download raw bytes from a URL (lightweight, no progress bar).
fn download_bytes(url_str: &str) -> anyhow::Result<Vec<u8>> {
    let agent = ureq::AgentBuilder::new()
        .user_agent("aminos/0.1")
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(30))
        .build();

    let resp = agent.get(url_str).call()
        .with_context(|| format!("下载失败: {}", url_str))?;

    if resp.status() >= 400 {
        bail!("HTTP {}", resp.status());
    }

    let mut reader = resp.into_reader();
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    Ok(buf)
}

/// Read a single software definition. Supports:
/// 1. Exact filename match
/// 2. Case-insensitive filename match
/// 3. Match by `display_name`
/// 4. Match by `aliases`
pub fn read_software_def(name: &str) -> anyhow::Result<SoftwareDef> {
    let source = paths::source_dir();
    let lower = name.to_lowercase();

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

/// List all available software definitions.
pub fn list_software_defs() -> anyhow::Result<Vec<SoftwareDef>> {
    let source = paths::source_dir();
    if !source.is_dir() {
        return Ok(Vec::new());
    }

    let mut defs: Vec<SoftwareDef> = Vec::new();
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&source) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().map_or(false, |e| e == "json") {
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
    fs::write(paths::installed_json(), json)?;
    Ok(())
}

pub fn record_installation(name: &str, version: &str, install_path: &str, version_provenance: &str, source_version: &str) -> anyhow::Result<()> {
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
