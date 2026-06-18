use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// 单个第三方源配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SourceEntry {
    pub name: String,
    pub url: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

/// sources.toml 顶层结构
#[derive(Debug, Clone, Deserialize, Serialize)]
struct SourcesFile {
    #[serde(default)]
    source: Vec<SourceEntry>,
}

/// 第三方源配置管理器。
///
/// 配置存储在 `%LOCALAPPDATA%/aminos/config/sources.toml`。
pub struct SourceConfig {
    path: PathBuf,
}

impl SourceConfig {
    /// 创建配置管理器，配置路径由调用方指定。
    pub fn new(config_dir: PathBuf) -> Self {
        Self { path: config_dir.join("sources.toml") }
    }

    /// 加载配置（文件不存在返回空列表）。
    pub fn load(&self) -> Vec<SourceEntry> {
        if !self.path.is_file() {
            return Vec::new();
        }
        match fs::read_to_string(&self.path) {
            Ok(content) => {
                match toml::from_str::<SourcesFile>(&content) {
                    Ok(cfg) => cfg.source,
                    Err(e) => {
                        eprintln!("  解析 sources.toml 失败: {}", e);
                        Vec::new()
                    }
                }
            }
            Err(e) => {
                eprintln!("  读取 sources.toml 失败: {}", e);
                Vec::new()
            }
        }
    }

    /// 保存配置。
    fn save(&self, entries: &[SourceEntry]) -> anyhow::Result<()> {
        let file = SourcesFile { source: entries.to_vec() };
        let toml_str = toml::to_string_pretty(&file)?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, toml_str)?;
        Ok(())
    }

    /// 添加一个第三方源。
    pub fn add(&self, name: &str, url: &str) -> anyhow::Result<()> {
        let mut entries = self.load();
        // 检查是否已存在同名源
        if entries.iter().any(|e| e.name == name) {
            anyhow::bail!("源 '{}' 已存在", name);
        }
        entries.push(SourceEntry {
            name: name.to_string(),
            url: url.to_string(),
            enabled: true,
        });
        self.save(&entries)?;
        println!("  已添加源: {} ({})", name, url);
        Ok(())
    }

    /// 移除一个第三方源。
    pub fn remove(&self, name: &str) -> anyhow::Result<()> {
        let mut entries = self.load();
        let len_before = entries.len();
        entries.retain(|e| e.name != name);
        if entries.len() == len_before {
            anyhow::bail!("未找到源 '{}'", name);
        }
        self.save(&entries)?;
        println!("  已移除源: {}", name);

        // 清理本地缓存的源文件
        let _ = std::fs::remove_dir_all(
            crate::paths::PathResolver::aminos()
                .appdata_root()
                .join("source").join("community").join(name)
        );
        Ok(())
    }

    /// 列出所有已配置的源。
    pub fn list(&self) -> Vec<SourceEntry> {
        self.load()
    }

    /// 切换源启用/禁用
    pub fn toggle(&self, name: &str, enabled: bool) -> anyhow::Result<()> {
        let mut entries = self.load();
        let action = if enabled { "启用" } else { "禁用" };
        match entries.iter_mut().find(|e| e.name == name) {
            Some(entry) => {
                entry.enabled = enabled;
                self.save(&entries)?;
                println!("  已{}源: {}", action, name);
                Ok(())
            }
            None => anyhow::bail!("未找到源 '{}'", name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_source_file_parsing() {
        let toml_str = r#"
[[source]]
name = "test-repo"
url = "https://example.com/repo"
enabled = true
"#;
        let parsed: SourcesFile = toml::from_str(toml_str).unwrap();
        assert_eq!(parsed.source.len(), 1);
        assert_eq!(parsed.source[0].name, "test-repo");
        assert_eq!(parsed.source[0].url, "https://example.com/repo");
        assert!(parsed.source[0].enabled);
    }

    #[test]
    fn test_empty_file() {
        let parsed: SourcesFile = toml::from_str("").unwrap();
        assert!(parsed.source.is_empty());
    }

    #[test]
    fn test_default_enabled() {
        let toml_str = r#"
[[source]]
name = "test"
url = "https://example.com"
"#;
        let parsed: SourcesFile = toml::from_str(toml_str).unwrap();
        assert!(parsed.source[0].enabled);
    }
}
