use std::path::PathBuf;

use crate::backend::{Aria2cBackend, BitsBackend, CurlBackend, DownloadBackend, PowerShellBackend, PowerShellInvokeBackend, RustRangeBackend, UreqBackend};

/// 配置文件路径：`%LOCALAPPDATA%\aminos\config\downloader.toml`
pub fn config_file_path() -> PathBuf {
    let local = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    local.join("aminos").join("config").join("downloader.toml")
}

/// 列出配置文件中的所有后端及其状态。
/// 如果配置文件不存在，写入默认配置文件后返回默认列表。
pub fn list_backend_states() -> Vec<(String, bool, Option<u8>)> {
    let path = config_file_path();
    if !path.is_file() {
        // 没有配置文件 → 写入默认配置，方便用户自行修改
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let default_toml = TomlConfig::default();
        if let Ok(content) = toml::to_string_pretty(&default_toml) {
            let _ = std::fs::write(&path, &content);
        }
        eprintln!("  已创建默认配置文件: {}", path.display());
        return vec![
            ("aria2c".into(), true, None),
            ("rust-range".into(), true, Some(16)),
            ("rust-ureq".into(), true, None),
            ("rust-ureq(insecure)".into(), true, None),
            ("powershell".into(), true, None),
            ("ps-invoke".into(), true, None),
            ("bits".into(), true, None),
            ("curl".into(), true, None),
        ];
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let parsed: Result<TomlConfig, _> = toml::from_str(&content);
    match parsed {
        Ok(cfg) => cfg.download.strategies.into_iter()
            .map(|s| (s.name, s.enabled, s.threads))
            .collect(),
        Err(_) => vec![],
    }
}

/// 查找后端（大小写不敏感），返回配置中的标准名称。
pub fn find_backend_name(name: &str) -> String {
    let states = list_backend_states();
    for (n, _, _) in &states {
        if n.eq_ignore_ascii_case(name) {
            return n.clone();
        }
    }
    name.to_string()
}

/// 修改配置文件中指定后端的启用状态（大小写不敏感）。
/// 如果配置文件不存在，先用默认值创建。
pub fn set_backend_enabled(name: &str, enabled: bool) -> anyhow::Result<()> {
    let path = config_file_path();

    // 确保父目录存在
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // 读取或创建默认配置
    let mut config: TomlConfig = if path.is_file() {
        let content = std::fs::read_to_string(&path)?;
        toml::from_str(&content).unwrap_or_default()
    } else {
        TomlConfig::default()
    };

    // 大小写不敏感查找
    let mut found = false;
    for s in &mut config.download.strategies {
        if s.name.eq_ignore_ascii_case(name) {
            s.enabled = enabled;
            found = true;
            break;
        }
    }

    if !found {
        anyhow::bail!("未找到后端: {}（可用后端: {}）", name,
            config.download.strategies.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", "));
    }

    // 写回文件
    let toml_str = toml::to_string(&config)?;
    std::fs::write(&path, toml_str)?;
    Ok(())
}

/// 初始化（如果不存在则创建）默认配置文件。
pub fn ensure_config_file() -> anyhow::Result<()> {
    let path = config_file_path();
    if path.is_file() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let toml_str = toml::to_string(&TomlConfig::default())?;
    std::fs::write(&path, toml_str)?;
    Ok(())
}

/// 下载配置文件结构。
#[derive(Debug)]
pub struct DownloaderConfig {
    /// 最大并发数（PermitPool）。
    pub max_concurrent: usize,
    /// 是否校验下载文件。
    pub verify: bool,
    /// 是否启用续传。
    pub resume: bool,
    /// 后端列表（按优先级排序，仅包含 enabled 的后端）。
    pub backends: Vec<Box<dyn DownloadBackend>>,
}

impl Default for DownloaderConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 16,
            verify: true,
            resume: true,
            backends: default_backends(),
        }
    }
}

impl DownloaderConfig {
    /// 加载配置：优先从文件读取，文件不存在则写入默认配置后返回默认值。
    pub fn load() -> Self {
        let config_path = config_file_path();

        if !config_path.exists() {
            // 写入默认配置文件，方便用户自行修改
            if let Some(parent) = config_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let default_toml = TomlConfig::default();
            if let Ok(content) = toml::to_string_pretty(&default_toml) {
                let _ = std::fs::write(&config_path, &content);
            }
            eprintln!("  已创建默认配置文件: {}", config_path.display());
            return Self::default();
        }

        let content = match std::fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("    读取配置文件失败（{}），使用默认配置: {}", config_path.display(), e);
                return Self::default();
            }
        };

        match toml::from_str::<TomlConfig>(&content) {
            Ok(mut parsed) => {
                // 自动补缺：检查是否有新的默认后端未在配置文件中
                sync_strategies_if_needed(&config_path, &mut parsed);
                let cfg = Self::from_toml_config(&parsed);
                eprintln!("    已加载配置文件: {}", config_path.display());
                cfg
            }
            Err(e) => {
                eprintln!("    解析配置文件失败: {}，使用默认配置", e);
                Self::default()
            }
        }
    }

    /// 从已解析的 TomlConfig 构建 DownloaderConfig。
    fn from_toml_config(parsed: &TomlConfig) -> Self {
        let mut backends = Vec::new();

        for entry in &parsed.download.strategies {
            if !entry.enabled {
                continue;
            }
            match entry.name.as_str() {
                "rust-range" | "RustRange" => {
                    backends.push(Box::new(RustRangeBackend {
                        threads: entry.threads.unwrap_or(16),
                        resume: entry.resume.unwrap_or(true),
                    }) as Box<dyn DownloadBackend>);
                }
                "aria2c" | "Aria2c" => backends.push(Box::new(Aria2cBackend)),
                "rust-ureq" | "Ureq" => backends.push(Box::new(UreqBackend::normal())),
                "rust-ureq(insecure)" | "UreqInsecure" => backends.push(Box::new(UreqBackend::insecure())),
                "powershell" | "PowerShell" => backends.push(Box::new(PowerShellBackend)),
                "ps-invoke" | "PowerShellInvoke" => backends.push(Box::new(PowerShellInvokeBackend)),
                "bits" | "BitsTransfer" => backends.push(Box::new(BitsBackend)),
                "curl" | "Curl" => backends.push(Box::new(CurlBackend)),
                "RustSingle" => {
                    // RustSingle 已合并到 RustRange，静默跳过
                }
                other => {
                    eprintln!("  配置文件包含未知后端: {}", other);
                }
            }
        }

        if backends.is_empty() {
            eprintln!("  配置文件中没有启用的后端，使用默认配置");
            return Self::default();
        }

        backends.sort_by_key(|b| b.priority());

        DownloaderConfig {
            max_concurrent: parsed.download.max_concurrent.unwrap_or(16).max(1).min(64),
            verify: parsed.download.verify.unwrap_or(true),
            resume: parsed.download.resume.unwrap_or(true),
            backends,
        }
    }

}

fn default_backends() -> Vec<Box<dyn DownloadBackend>> {
    vec![
        Box::new(Aria2cBackend),
        Box::new(RustRangeBackend::default()),
        Box::new(UreqBackend::normal()),
        Box::new(UreqBackend::insecure()),
        Box::new(PowerShellBackend),
        Box::new(PowerShellInvokeBackend),
        Box::new(BitsBackend),
        Box::new(CurlBackend),
    ]
}

// ── TOML 解析中间结构 ───────────────────────────────

#[derive(serde::Serialize, serde::Deserialize)]
struct TomlConfig {
    #[serde(default)]
    download: TomlDownload,
}

impl Default for TomlConfig {
    fn default() -> Self {
        Self {
            download: TomlDownload {
                max_concurrent: Some(16),
                verify: Some(true),
                resume: Some(true),
                strategies: default_strategies(),
            },
        }
    }
}

/// 默认的后端策略列表（代码维护的权威列表）。
fn default_strategies() -> Vec<TomlStrategy> {
    vec![
        TomlStrategy { name: "aria2c".into(), enabled: true, threads: None, resume: None },
        TomlStrategy { name: "rust-range".into(), enabled: true, threads: Some(16), resume: Some(true) },
        TomlStrategy { name: "rust-ureq".into(), enabled: true, threads: None, resume: None },
        TomlStrategy { name: "rust-ureq(insecure)".into(), enabled: true, threads: None, resume: None },
        TomlStrategy { name: "powershell".into(), enabled: true, threads: None, resume: None },
        TomlStrategy { name: "ps-invoke".into(), enabled: true, threads: None, resume: None },
        TomlStrategy { name: "bits".into(), enabled: true, threads: None, resume: None },
        TomlStrategy { name: "curl".into(), enabled: true, threads: None, resume: None },
    ]
}

/// 检查现有策略列表是否缺失默认后端，如有则追加并写回文件。
fn sync_strategies_if_needed(config_path: &std::path::Path, parsed: &mut TomlConfig) {
    let existing_names: std::collections::HashSet<String> = parsed.download.strategies.iter()
        .map(|s| s.name.clone())
        .collect();
    let mut changed = false;
    for def in default_strategies() {
        if !existing_names.contains(&def.name) {
            eprintln!("  配置文件中缺少后端「{}」，已自动添加", def.name);
            parsed.download.strategies.push(def);
            changed = true;
        }
    }
    if changed {
        if let Ok(content) = toml::to_string_pretty(&parsed) {
            let _ = std::fs::write(config_path, content);
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct TomlDownload {
    #[serde(skip_serializing_if = "Option::is_none")]
    max_concurrent: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    verify: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resume: Option<bool>,
    #[serde(default)]
    strategies: Vec<TomlStrategy>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct TomlStrategy {
    name: String,
    enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    threads: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resume: Option<bool>,
}
