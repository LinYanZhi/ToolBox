use std::path::PathBuf;

use serde::Deserialize;

/// 文件扩展名 → 颜色映射
pub type ExtColorMap = Vec<(String, String)>;

/// 大小阈值规则
#[derive(Debug, Clone)]
pub struct SizeRule {
    pub max: i64,
    pub color: String,
    pub mode: String,
}

/// 运行时颜色配置
pub struct ColorConfig {
    /// 普通目录颜色
    pub dir_color: String,
    /// 链接目录颜色
    pub dir_link_color: String,
    /// 目录链接箭头
    pub dir_link_arrow: String,
    /// 目录链接箭头颜色
    pub dir_link_arrow_color: String,
    /// 目录链接路径颜色
    pub dir_link_path_color: String,
    /// 目录链接路径中的目录名颜色
    pub dir_link_path_basename_color: String,

    /// 文件扩展名 → 颜色
    pub file_extensions: ExtColorMap,
    /// 文件链接箭头
    pub file_link_arrow: String,
    /// 文件链接箭头颜色
    pub file_link_arrow_color: String,
    /// 文件链接指向目录时的目录名颜色
    pub file_link_dir_color: String,

    /// 文件大小颜色规则
    pub size_rules: Vec<SizeRule>,
}

impl ColorConfig {
    /// 初始化 ANSI 支持
    pub fn init() {
        color::enable_ansi();
    }

    /// 加载配置：优先读取 YAML 文件，不存在则使用代码默认值并自动创建 YAML 文件
    pub fn load() -> Self {
        let path = get_config_path();
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                match serde_yaml::from_str::<LsConfigFile>(&content) {
                    Ok(cfg) => cfg.into_color_config(),
                    Err(_) => {
                        eprintln!("{} ls.yaml 格式有误，使用默认配置", color::yellow("注意:"));
                        ColorConfig::default()
                    }
                }
            }
            Err(_) => {
                // 文件不存在，创建默认配置
                let created = ensure_config();
                eprintln!("{} 已创建默认配置文件: {}", color::yellow("注意:"), created.display());
                ColorConfig::default()
            }
        }
    }
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            dir_color: "96".into(),
            dir_link_color: "36".into(),
            dir_link_arrow: "=>".into(),
            dir_link_arrow_color: "90".into(),
            dir_link_path_color: "90".into(),
            dir_link_path_basename_color: "96".into(),

            file_extensions: vec![
                (".7z".into(),   "31".into()),
                (".zip".into(),  "31".into()),
                (".rar".into(),  "31".into()),
                (".tar".into(),  "31".into()),
                (".gz".into(),   "31".into()),
                (".bz2".into(),  "31".into()),
                (".xz".into(),   "31".into()),
                (".exe".into(),  "32".into()),
                (".msi".into(),  "32".into()),
                (".bat".into(),  "32".into()),
                (".cmd".into(),  "32".into()),
                (".py".into(),   "93".into()),
                (".rs".into(),   "33".into()),
                (".js".into(),   "33".into()),
                (".ts".into(),   "33".into()),
                (".html".into(), "35".into()),
                (".css".into(),  "35".into()),
                (".json".into(), "37".into()),
                (".toml".into(), "37".into()),
                (".yaml".into(), "37".into()),
                (".yml".into(),  "37".into()),
                (".md".into(),   "37".into()),
                (".txt".into(),  "37".into()),
                (".lnk".into(),  "94".into()),
                (".dll".into(),  "90".into()),
                (".pdb".into(),  "90".into()),
                (".dat".into(),  "90".into()),
                (".ini".into(),  "90".into()),
                (".lock".into(), "90".into()),
                (".log".into(),  "90".into()),
            ],
            file_link_arrow: "->".into(),
            file_link_arrow_color: "90".into(),
            file_link_dir_color: "96".into(),

            size_rules: vec![
                SizeRule { max: 1024, color: "90".into(), mode: "full".into() },
                SizeRule { max: 1_048_576, color: "90".into(), mode: "unit".into() },
                SizeRule { max: 104_857_600, color: "93".into(), mode: "unit".into() },
                SizeRule { max: 1_073_741_824, color: "93".into(), mode: "full".into() },
                SizeRule { max: 2_147_483_648, color: "91".into(), mode: "unit".into() },
                SizeRule { max: -1, color: "91".into(), mode: "full".into() },
            ],
        }
    }
}

impl ColorConfig {
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

// ── YAML 配置文件支持 ──────────────────────────

/// ls.yaml 顶层结构
#[derive(Debug, Deserialize)]
struct LsConfigFile {
    #[serde(default)]
    dir: DirConfig,
    #[serde(default)]
    file: FileLinkConfig,
    #[serde(default)]
    extensions: Vec<ExtEntry>,
    #[serde(default)]
    size_rules: Vec<SizeEntry>,
}

#[derive(Debug, Deserialize)]
struct DirConfig {
    #[serde(default = "default_dir_color")]
    color: String,
    #[serde(default = "default_dir_link_color")]
    link_color: String,
    #[serde(default = "default_dir_link_arrow")]
    link_arrow: String,
    #[serde(default = "default_dir_link_arrow_color")]
    link_arrow_color: String,
    #[serde(default = "default_dir_link_path_color")]
    link_path_color: String,
    #[serde(default = "default_dir_link_path_basename_color")]
    link_path_basename_color: String,
}

impl Default for DirConfig {
    fn default() -> Self {
        Self {
            color: default_dir_color(),
            link_color: default_dir_link_color(),
            link_arrow: default_dir_link_arrow(),
            link_arrow_color: default_dir_link_arrow_color(),
            link_path_color: default_dir_link_path_color(),
            link_path_basename_color: default_dir_link_path_basename_color(),
        }
    }
}

fn default_dir_color() -> String { "96".into() }
fn default_dir_link_color() -> String { "36".into() }
fn default_dir_link_arrow() -> String { "=>".into() }
fn default_dir_link_arrow_color() -> String { "90".into() }
fn default_dir_link_path_color() -> String { "90".into() }
fn default_dir_link_path_basename_color() -> String { "96".into() }

#[derive(Debug, Deserialize)]
struct FileLinkConfig {
    #[serde(default = "default_file_link_arrow")]
    arrow: String,
    #[serde(default = "default_file_link_arrow_color")]
    arrow_color: String,
    #[serde(default = "default_file_link_dir_color")]
    dir_color: String,
}

impl Default for FileLinkConfig {
    fn default() -> Self {
        Self {
            arrow: default_file_link_arrow(),
            arrow_color: default_file_link_arrow_color(),
            dir_color: default_file_link_dir_color(),
        }
    }
}

fn default_file_link_arrow() -> String { "->".into() }
fn default_file_link_arrow_color() -> String { "90".into() }
fn default_file_link_dir_color() -> String { "96".into() }

#[derive(Debug, Deserialize)]
struct ExtEntry {
    ext: String,
    color: String,
}

#[derive(Debug, Deserialize)]
struct SizeEntry {
    max: i64,
    color: String,
    #[serde(default = "default_size_mode")]
    mode: String,
}

fn default_size_mode() -> String { "full".into() }

impl LsConfigFile {
    fn into_color_config(self) -> ColorConfig {
        ColorConfig {
            dir_color: self.dir.color,
            dir_link_color: self.dir.link_color,
            dir_link_arrow: self.dir.link_arrow,
            dir_link_arrow_color: self.dir.link_arrow_color,
            dir_link_path_color: self.dir.link_path_color,
            dir_link_path_basename_color: self.dir.link_path_basename_color,

            file_extensions: self.extensions.iter().map(|e| (e.ext.clone(), e.color.clone())).collect(),

            file_link_arrow: self.file.arrow,
            file_link_arrow_color: self.file.arrow_color,
            file_link_dir_color: self.file.dir_color,

            size_rules: self.size_rules.iter().map(|s| SizeRule {
                max: s.max,
                color: s.color.clone(),
                mode: s.mode.clone(),
            }).collect(),
        }
    }
}

/// 获取配置目录路径（%LOCALAPPDATA%\ls\）
pub fn get_config_dir() -> PathBuf {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(local).join("ls")
}

/// 获取 ls.yaml 路径
fn get_config_path() -> PathBuf {
    get_config_dir().join("ls.yaml")
}

/// 确保配置文件存在，不存在则创建默认配置
pub fn ensure_config() -> PathBuf {
    let path = get_config_path();
    if !path.exists() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let yaml = generate_default_yaml();
        let _ = std::fs::write(&path, &yaml);
    }
    path
}

/// 清除配置文件，下次运行时自动重新创建默认配置
/// 返回 true 表示确实删除了文件
pub fn clear_config() -> bool {
    let path = get_config_path();
    if path.exists() {
        let _ = std::fs::remove_file(&path);
        true
    } else {
        false
    }
}

/// 生成默认 YAML 配置内容
fn generate_default_yaml() -> String {
    r#"# ls 配色配置
# 首次运行时自动生成，可自由修改
# 颜色值使用 ANSI 数字码:
#   30=黑 31=红 32=绿 33=黄 34=蓝 35=紫 36=青 37=白
#   90=灰 91=亮红 92=亮绿 93=亮黄 94=亮蓝 95=亮紫 96=亮青 97=亮白

# 目录样式
dir:
  color: "96"                # 普通目录
  link_color: "36"           # 符号链接/目录连接点
  link_arrow: "=>"           # 目录链接箭头
  link_arrow_color: "90"     # 目录链接箭头颜色
  link_path_color: "90"      # 目录链接路径颜色
  link_path_basename_color: "96"  # 目录链接路径中的目录名颜色

# 文件链接样式
file:
  arrow: "->"                # 文件链接箭头
  arrow_color: "90"          # 文件链接箭头颜色
  dir_color: "96"            # 文件链接指向目录时的目录名颜色

# 文件扩展名配色
extensions:
  - { ext: ".7z",   color: "31" }
  - { ext: ".zip",  color: "31" }
  - { ext: ".rar",  color: "31" }
  - { ext: ".tar",  color: "31" }
  - { ext: ".gz",   color: "31" }
  - { ext: ".bz2",  color: "31" }
  - { ext: ".xz",   color: "31" }
  - { ext: ".exe",  color: "32" }
  - { ext: ".msi",  color: "32" }
  - { ext: ".bat",  color: "32" }
  - { ext: ".cmd",  color: "32" }
  - { ext: ".py",   color: "93" }
  - { ext: ".rs",   color: "33" }
  - { ext: ".js",   color: "33" }
  - { ext: ".ts",   color: "33" }
  - { ext: ".html", color: "35" }
  - { ext: ".css",  color: "35" }
  - { ext: ".json", color: "37" }
  - { ext: ".toml", color: "37" }
  - { ext: ".yaml", color: "37" }
  - { ext: ".yml",  color: "37" }
  - { ext: ".md",   color: "37" }
  - { ext: ".txt",  color: "37" }
  - { ext: ".lnk",  color: "94" }
  - { ext: ".dll",  color: "90" }
  - { ext: ".pdb",  color: "90" }
  - { ext: ".dat",  color: "90" }
  - { ext: ".ini",  color: "90" }
  - { ext: ".lock", color: "90" }
  - { ext: ".log",  color: "90" }

# 文件大小颜色规则
# mode: full=整体着色, unit=仅单位部分着色
# max: 字节阈值（-1=其余所有）
size_rules:
  - { max: 1024,        color: "90", mode: full }
  - { max: 1048576,     color: "90", mode: unit }
  - { max: 104857600,   color: "93", mode: unit }
  - { max: 1073741824,  color: "93", mode: full }
  - { max: 2147483648,  color: "91", mode: unit }
  - { max: -1,          color: "91", mode: full }
"#.to_string()
}
