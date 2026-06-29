use std::collections::HashMap;
use serde::Deserialize;

const EMBEDDED_SOURCE: &str = include_str!("../source.json");

#[derive(Debug, Clone, Deserialize)]
pub struct DetectConfig {
    pub display_name: String,
    #[serde(default)]
    pub publisher: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionEntry {
    #[serde(default)]
    pub registry_version: Option<String>,
    pub urls: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoftwareEntry {
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub detect: Option<DetectConfig>,
    pub versions: HashMap<String, VersionEntry>,
}

pub fn all_entries() -> anyhow::Result<HashMap<String, SoftwareEntry>> {
    serde_json::from_str(EMBEDDED_SOURCE)
        .map_err(|e| anyhow::anyhow!("解析源数据失败: {}", e))
}

pub fn resolve(name: &str) -> Option<(String, SoftwareEntry)> {
    let all = all_entries().ok()?;
    let lower = name.to_lowercase();

    if let Some(entry) = all.get(&lower) {
        return Some((lower, entry.clone()));
    }

    for (key, entry) in &all {
        if entry.aliases.iter().any(|a| a.to_lowercase() == lower) {
            return Some((key.clone(), entry.clone()));
        }
    }

    None
}

#[derive(Debug, Clone)]
pub struct RegistryInfo {
    pub display_name: String,
    pub version: String,
    pub install_path: Option<String>,
    pub uninstall_string: Option<String>,
}

pub fn detect_from_registry(detect: &DetectConfig) -> Option<RegistryInfo> {
    let cmd = format!(
        "Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*', \
         'HKLM:\\SOFTWARE\\Wow6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*', \
         'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*' 2>$null | \
         Where-Object {{ $_.DisplayName -like '*{}*' }} | Select-Object -First 1",
        detect.display_name
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &cmd])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut info = RegistryInfo {
        display_name: String::new(),
        version: String::new(),
        install_path: None,
        uninstall_string: None,
    };

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 2 {
            let key = parts[0].trim().to_lowercase();
            let value: String = parts[1..].join(":").trim().to_string();
            match key.as_str() {
                "displayname" => info.display_name = value,
                "displayversion" => info.version = value,
                "installlocation" => info.install_path = Some(value),
                "uninstallstring" => info.uninstall_string = Some(value),
                _ => {}
            }
        }
    }

    if !info.display_name.is_empty() {
        Some(info)
    } else {
        None
    }
}