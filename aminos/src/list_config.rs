use std::path::PathBuf;

use crate::paths;

/// 列表显示的过滤配置。
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ListConfig {
    /// 按显示名称关键词过滤（支持前缀匹配，末尾 `*` 表示前缀）。
    pub hide_display_names: Vec<String>,
}

impl Default for ListConfig {
    fn default() -> Self {
        Self {
            hide_display_names: vec![
                // Visual C++ Redistributables
                "Microsoft Visual C++*".into(),
                "Microsoft Visual C++ v14*".into(),
                "vcpp_crt*".into(),
                // Windows SDK 及子组件
                "Windows SDK*".into(),
                "Windows * Extension SDK*".into(),
                "Windows Software Development Kit*".into(),
                "Windows App Certification Kit*".into(),
                "Windows Application Driver*".into(),
                "Windows Desktop*".into(),
                "Windows IoT*".into(),
                "Windows Mobile*".into(),
                "Windows Team*".into(),
                "WinRT Intellisense*".into(),
                "WinAppDeploy*".into(),
                // Universal CRT
                "Universal CRT*".into(),
                "Universal General MIDI*".into(),
                // SDK ARM
                "SDK ARM64*".into(),
                "SDK ARM*".into(),
                // Visual Studio 内部组件
                "vs_*".into(),
                "VS *".into(),
                "vs_CoreEditor*".into(),
                "Visual Studio 生成工具*".into(),
                // Office Click-to-Run 子组件
                "Office 16 Click-to-Run*".into(),
                // 系统工具 & 运行时
                "Microsoft Update Health*".into(),
                "Microsoft System CLR Types*".into(),
                "MSI Development Tools*".into(),
                "Kits Configuration Installer*".into(),
                "DiagnosticsHub*".into(),
                "IntelliTrace*".into(),
                "icecap_collection*".into(),
                // 性能 & 调试工具
                "WPT*".into(),
                "Application Verifier*".into(),
                // 其他
                "Update for Windows*".into(),
                "Rustup*".into(),
            ],
        }
    }
}

impl ListConfig {
    /// 配置文件路径：`%LOCALAPPDATA%\aminos\config\list.json`
    fn config_path() -> PathBuf {
        paths::config_dir().join("list.json")
    }

    /// 加载配置，文件不存在时写入默认配置。
    pub fn load() -> Self {
        let path = Self::config_path();
        if !path.is_file() {
            let cfg = Self::default();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(content) = serde_json::to_string_pretty(&cfg) {
                let _ = std::fs::write(&path, content);
            }
            return cfg;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        serde_json::from_str(&content).unwrap_or_default()
    }

    /// 检查显示名称是否被过滤规则命中。
    pub fn is_hidden(&self, display_name: &str) -> bool {
        self.hide_display_names.iter().any(|pattern| {
            if let Some(prefix) = pattern.strip_suffix('*') {
                // 前缀匹配：Microsoft Visual C++* → 匹配 Microsoft Visual C++ 2013...
                display_name.starts_with(prefix)
            } else {
                // 精确匹配
                display_name == pattern
            }
        })
    }
}
