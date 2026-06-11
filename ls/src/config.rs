use serde::Deserialize;
use std::path::Path;

/// 文件扩展名 → 颜色映射
pub type ExtColorMap = Vec<(String, String)>;

/// 大小阈值规则
#[derive(Debug, Clone, Deserialize)]
pub struct SizeRule {
    pub max: i64,
    pub color: String,
    pub mode: String,
}

/// YAML 配置顶层结构
#[derive(Debug, Deserialize)]
pub struct DirConfig {
    #[serde(rename = "type-color")]
    pub type_color: Option<String>,
    pub basename: Option<String>,
    #[serde(rename = "python-env")]
    pub python_env: Option<String>,
    #[serde(rename = "java-env")]
    pub java_env: Option<String>,
    #[serde(rename = "link-basename")]
    pub link_basename: Option<String>,
    pub link: Option<String>,
    #[serde(rename = "link-color")]
    pub link_color: Option<String>,
    #[serde(rename = "link-path")]
    pub link_path: Option<String>,
    #[serde(rename = "link-path-basename")]
    pub link_path_basename: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FileConfigOther {
    #[serde(flatten)]
    pub items: std::collections::HashMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct FileConfig {
    #[serde(rename = "type-color")]
    pub type_color: Option<String>,
    #[serde(rename = "color-range")]
    pub color_range: Option<String>,
    pub basename: Option<String>,
    pub other: Option<FileConfigOther>,
    pub link: Option<String>,
    #[serde(rename = "link-color")]
    pub link_color: Option<String>,
    #[serde(rename = "link-path")]
    pub link_path: Option<String>,
    #[serde(rename = "link-dir")]
    pub link_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LsConfig {
    pub dir: Option<DirConfig>,
    pub file: Option<FileConfig>,
    pub size: Option<Vec<SizeRule>>,
}

/// 运行时颜色配置（解析后的扁平结构）
pub struct ColorConfig {
    /// 目录类型标记颜色
    pub dir_type_color: Option<String>,
    /// 普通目录颜色
    pub dir_basename: Option<String>,
    /// Python 环境目录颜色
    pub python_env: Option<String>,
    /// Java 环境目录颜色
    pub java_env: Option<String>,
    /// 链接目录颜色
    pub dir_link_basename: Option<String>,
    /// 目录链接箭头样式
    pub dir_link_arrow: String,
    /// 目录链接箭头颜色
    pub dir_link_arrow_color: Option<String>,
    /// 目录链接路径颜色
    pub dir_link_path: Option<String>,
    /// 目录链接路径中的目录名颜色
    pub dir_link_path_basename: Option<String>,

    /// 文件类型标记颜色
    pub file_type_color: Option<String>,
    /// 颜色范围: "suffix" 或 "filename"
    pub color_range: String,
    /// 普通文件颜色
    pub file_basename: Option<String>,
    /// 文件扩展名 → 颜色
    pub file_extensions: ExtColorMap,
    /// 文件链接箭头样式
    pub file_link_arrow: String,
    /// 文件链接箭头颜色
    pub file_link_arrow_color: Option<String>,
    /// 文件链接路径颜色
    pub file_link_path: Option<String>,
    /// 文件链接指向目录时的目录名颜色
    pub file_link_dir: Option<String>,

    /// 文件大小颜色规则
    pub size_rules: Vec<SizeRule>,
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            dir_type_color: Some("gray".into()),
            dir_basename: Some("lightcyan".into()),
            python_env: Some("lightyellow".into()),
            java_env: Some("brightwhite".into()),
            dir_link_basename: Some("cyan".into()),
            dir_link_arrow: "=>".into(),
            dir_link_arrow_color: Some("gray".into()),
            dir_link_path: Some("gray".into()),
            dir_link_path_basename: Some("lightcyan".into()),
            file_type_color: Some("gray".into()),
            color_range: "suffix".into(),
            file_basename: Some("white".into()),
            file_extensions: vec![
                (".dll".into(), "gray".into()),
                (".dat".into(), "gray".into()),
                (".ini".into(), "gray".into()),
                (".lock".into(), "gray".into()),
                (".exe".into(), "green".into()),
                (".bat".into(), "green".into()),
                (".cmd".into(), "green".into()),
                (".7z".into(), "red".into()),
                (".zip".into(), "red".into()),
                (".tar".into(), "red".into()),
                (".rar".into(), "red".into()),
                (".apk".into(), "red".into()),
                (".jar".into(), "red".into()),
                (".gz".into(), "red".into()),
                (".lnk".into(), "lightblue".into()),
                (".py".into(), "yellow".into()),
            ],
            file_link_arrow: "->".into(),
            file_link_arrow_color: Some("gray".into()),
            file_link_path: Some("gray".into()),
            file_link_dir: Some("lightcyan".into()),
            size_rules: vec![
                SizeRule { max: 1024, color: "gray".into(), mode: "full".into() },
                SizeRule { max: 1_048_576, color: "gray".into(), mode: "unit".into() },
                SizeRule { max: 104_857_600, color: "yellow".into(), mode: "unit".into() },
                SizeRule { max: 1_073_741_824, color: "yellow".into(), mode: "full".into() },
                SizeRule { max: 2_147_483_648, color: "red".into(), mode: "unit".into() },
                SizeRule { max: -1, color: "red".into(), mode: "full".into() },
            ],
        }
    }
}

impl ColorConfig {
    /// 从 YAML 文件加载配置，失败时使用默认值
    pub fn from_yaml(path: &Path) -> Self {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        let parsed: LsConfig = match serde_yaml::from_str(&content) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        Self::from_parsed(parsed)
    }

    fn from_parsed(parsed: LsConfig) -> Self {
        let mut this = Self::default();

        if let Some(dir) = &parsed.dir {
            if let Some(v) = &dir.type_color { this.dir_type_color = Some(v.clone()); }
            if let Some(v) = &dir.basename { this.dir_basename = Some(v.clone()); }
            if let Some(v) = &dir.python_env { this.python_env = Some(v.clone()); }
            if let Some(v) = &dir.java_env { this.java_env = Some(v.clone()); }
            if let Some(v) = &dir.link_basename { this.dir_link_basename = Some(v.clone()); }
            if let Some(v) = &dir.link { this.dir_link_arrow = v.clone(); }
            if let Some(v) = &dir.link_color { this.dir_link_arrow_color = Some(v.clone()); }
            if let Some(v) = &dir.link_path { this.dir_link_path = Some(v.clone()); }
            if let Some(v) = &dir.link_path_basename { this.dir_link_path_basename = Some(v.clone()); }
        }

        if let Some(file) = &parsed.file {
            if let Some(v) = &file.type_color { this.file_type_color = Some(v.clone()); }
            if let Some(v) = &file.color_range { this.color_range = v.clone(); }
            if let Some(v) = &file.basename { this.file_basename = Some(v.clone()); }
            if let Some(v) = &file.link { this.file_link_arrow = v.clone(); }
            if let Some(v) = &file.link_color { this.file_link_arrow_color = Some(v.clone()); }
            if let Some(v) = &file.link_path { this.file_link_path = Some(v.clone()); }
            if let Some(v) = &file.link_dir { this.file_link_dir = Some(v.clone()); }

            if let Some(other) = &file.other {
                for (color, exts) in &other.items {
                    for ext in exts {
                        this.file_extensions.push((ext.clone(), color.clone()));
                    }
                }
            }
        }

        if let Some(sizes) = &parsed.size {
            this.size_rules = sizes.clone();
        }

        this
    }

    /// 根据扩展名获取颜色
    pub fn ext_color(&self, ext: &str) -> Option<&str> {
        let ext_lower = ext.to_lowercase();
        for (pattern, color) in &self.file_extensions {
            if pattern == &ext_lower {
                return Some(color);
            }
        }
        None
    }
}
