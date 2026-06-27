use std::collections::HashMap;
use serde::Deserialize;

/// 单一版本配置
#[derive(Debug, Clone, Deserialize)]
pub struct VersionEntry {
    /// 下载地址列表
    pub urls: Vec<String>,
}

/// 软件条目
#[derive(Debug, Clone, Deserialize)]
pub struct SoftwareEntry {
    /// 说明
    #[serde(default)]
    pub desc: String,
    /// 别名
    #[serde(default)]
    pub aliases: Vec<String>,
    /// 版本号 → 版本配置
    pub versions: HashMap<String, VersionEntry>,
}

// ── 嵌入式源

const EMBEDDED_SOURCE: &str = include_str!("../source.json");

/// 获取所有软件源条目
pub fn all_entries() -> anyhow::Result<HashMap<String, SoftwareEntry>> {
    serde_json::from_str(EMBEDDED_SOURCE)
        .map_err(|e| anyhow::anyhow!("解析源数据失败: {}", e))
}

/// 查找软件：name 精确匹配 → aliases 精确匹配
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
