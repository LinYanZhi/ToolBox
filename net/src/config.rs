use std::path::PathBuf;

use crate::backend::{Aria2cBackend, BitsBackend, CurlBackend, DownloadBackend, PowerShellBackend, PowerShellInvokeBackend, RustRangeBackend, UreqBackend};

/// 配置文件路径：`%LOCALAPPDATA%\aminos\config\download.toml`
pub fn config_file_path() -> PathBuf {
    let local = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    local.join("aminos").join("config").join("download.toml")
}

/// 列出配置文件中的所有后端及其状态。
/// 如果配置文件不存在，返回默认后端列表。
pub fn list_backend_states() -> Vec<(String, bool, Option<u8>)> {
    let path = config_file_path();
    if !path.is_file() {
        return vec![
            ("RustRange".into(), true, Some(16)),
            ("Aria2c".into(), true, None),
            ("Ureq".into(), true, None),
            ("UreqInsecure".into(), true, None),
            ("PowerShell".into(), true, None),
            ("PowerShellInvoke".into(), true, None),
            ("BitsTransfer".into(), true, None),
            ("Curl".into(), true, None),
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
    /// 加载配置：优先从文件读取，文件不存在则使用默认配置。
    pub fn load() -> Self {
        let config_path = config_file_path();

        if !config_path.exists() {
            return Self::default();
        }

        let content = match std::fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("    ⚠ 读取配置文件失败 ({}), 使用默认配置: {}", config_path.display(), e);
                return Self::default();
            }
        };

        match Self::from_toml(&content) {
            Ok(cfg) => {
                eprintln!("    ℹ 已加载配置文件: {}", config_path.display());
                cfg
            }
            Err(e) => {
                eprintln!("    ⚠ 解析配置文件失败: {}, 使用默认配置", e);
                Self::default()
            }
        }
    }

    /// 从 TOML 字符串解析配置。
    fn from_toml(content: &str) -> anyhow::Result<Self> {
        let parsed: TomlConfig = toml::from_str(content)?;
        let mut backends = Vec::new();

        for entry in parsed.download.strategies {
            if !entry.enabled {
                continue;
            }
            match entry.name.as_str() {
                "RustRange" => {
                    backends.push(Box::new(RustRangeBackend {
                        threads: entry.threads.unwrap_or(16),
                        resume: entry.resume.unwrap_or(true),
                    }) as Box<dyn DownloadBackend>);
                }
                "Aria2c" => backends.push(Box::new(Aria2cBackend)),
                "Ureq" => backends.push(Box::new(UreqBackend::normal())),
                "UreqInsecure" => backends.push(Box::new(UreqBackend::insecure())),
                "PowerShell" => backends.push(Box::new(PowerShellBackend)),
                "PowerShellInvoke" => backends.push(Box::new(PowerShellInvokeBackend)),
                "BitsTransfer" => backends.push(Box::new(BitsBackend)),
                "Curl" => backends.push(Box::new(CurlBackend)),
                other => {
                    eprintln!("    ⚠ 配置文件包含未知后端: {}", other);
                }
            }
        }

        if backends.is_empty() {
            anyhow::bail!("配置文件中没有启用的后端");
        }

        backends.sort_by_key(|b| b.priority());

        Ok(DownloaderConfig {
            max_concurrent: parsed.download.max_concurrent.unwrap_or(16).max(1).min(64),
            verify: parsed.download.verify.unwrap_or(true),
            resume: parsed.download.resume.unwrap_or(true),
            backends,
        })
    }
}

fn default_backends() -> Vec<Box<dyn DownloadBackend>> {
    vec![
        Box::new(RustRangeBackend::default()),
        Box::new(Aria2cBackend),
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
                strategies: vec![
                    TomlStrategy { name: "RustRange".into(), enabled: true, threads: Some(16), resume: Some(true) },
                    TomlStrategy { name: "Aria2c".into(), enabled: true, threads: None, resume: None },
                    TomlStrategy { name: "Ureq".into(), enabled: true, threads: None, resume: None },
                    TomlStrategy { name: "UreqInsecure".into(), enabled: true, threads: None, resume: None },
                    TomlStrategy { name: "PowerShell".into(), enabled: true, threads: None, resume: None },
                    TomlStrategy { name: "PowerShellInvoke".into(), enabled: true, threads: None, resume: None },
                    TomlStrategy { name: "BitsTransfer".into(), enabled: true, threads: None, resume: None },
                    TomlStrategy { name: "Curl".into(), enabled: true, threads: None, resume: None },
                ],
            },
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
