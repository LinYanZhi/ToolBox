use std::collections::HashMap;
use std::path::PathBuf;

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

// ── ls.yaml 配置（--set / --path）─────────────────────────

/// ls.yaml 顶层结构
#[derive(Debug, Default, serde::Deserialize)]
pub struct LsConfig {
    #[serde(default)]
    pub variables: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub variable_styles: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub paths: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub path_styles: HashMap<String, Vec<String>>,
}

/// 将 "颜色: [项目列表]" 转为 "项目: 颜色"
fn invert_map(input: &HashMap<String, Vec<String>>) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for (color, items) in input {
        for item in items {
            result.insert(item.clone(), color.clone());
        }
    }
    result
}

/// 获取 ls.yaml 配置文件的路径
///
/// 只查找 ls.exe 同级目录下的 `ls.yaml`，不依赖 aminos 环境。
fn get_ls_config_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        exe.parent().unwrap_or(std::path::Path::new(".")).join("ls.yaml")
    } else {
        PathBuf::from("ls.yaml")
    }
}

/// 加载 ls.yaml 配置
pub fn load_ls_config() -> LsConfig {
    let path = get_ls_config_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            serde_yaml::from_str(&content).unwrap_or_default()
        }
        Err(_) => LsConfig::default(),
    }
}

/// 内置默认环境变量配色
fn default_variable_colors() -> HashMap<String, String> {
    let mut m = HashMap::new();
    // 系统/路径类 → 青色
    m.insert("PATH".into(), "cyan".into());
    m.insert("PWD".into(), "cyan".into());
    m.insert("OLDPWD".into(), "cyan".into());
    m.insert("HOME".into(), "cyan".into());
    m.insert("USERPROFILE".into(), "cyan".into());
    // 身份类 → 绿色
    m.insert("USERNAME".into(), "green".into());
    m.insert("USER".into(), "green".into());
    m.insert("COMPUTERNAME".into(), "green".into());
    m.insert("HOSTNAME".into(), "green".into());
    // 系统目录 → 蓝色
    m.insert("SystemRoot".into(), "blue".into());
    m.insert("WINDIR".into(), "blue".into());
    m.insert("ProgramFiles".into(), "blue".into());
    m.insert("ProgramFiles(x86)".into(), "blue".into());
    m.insert("ProgramData".into(), "blue".into());
    m.insert("ALLUSERSPROFILE".into(), "blue".into());
    // 临时目录 → 黄色
    m.insert("TEMP".into(), "yellow".into());
    m.insert("TMP".into(), "yellow".into());
    // 开发环境 → 红色/紫色
    m.insert("JAVA_HOME".into(), "red".into());
    m.insert("JAVA_HOME_11".into(), "red".into());
    m.insert("JAVA_HOME_17".into(), "red".into());
    m.insert("JAVA_HOME_21".into(), "red".into());
    m.insert("GOROOT".into(), "red".into());
    m.insert("RUSTUP_HOME".into(), "red".into());
    m.insert("CARGO_HOME".into(), "red".into());
    // Shell 类 → 紫色
    m.insert("SHELL".into(), "purple".into());
    m.insert("EDITOR".into(), "purple".into());
    m.insert("VISUAL".into(), "purple".into());
    // 显示相关 → 浅蓝色
    m.insert("TERM".into(), "lightblue".into());
    m.insert("TERMINAL".into(), "lightblue".into());
    m.insert("COLORTERM".into(), "lightblue".into());
    m.insert("DISPLAY".into(), "lightblue".into());
    m
}

/// 内置默认 PATH 路径配色
fn default_path_colors() -> HashMap<String, String> {
    let mut m = HashMap::new();
    // 自研工具
    m.insert("*aminos*".into(), "cyan".into());
    m.insert("*\\.cargo\\*".into(), "cyan".into());
    m.insert("*\\.tool\\*".into(), "cyan".into());
    // 开发工具
    m.insert("*\\Python\\*".into(), "green".into());
    m.insert("*\\nodejs\\*".into(), "green".into());
    m.insert("*\\Git\\*".into(), "lightred".into());
    m.insert("*\\Java\\*".into(), "red".into());
    m.insert("*\\Go\\*".into(), "red".into());
    // 系统目录
    m.insert("*\\System32\\*".into(), "blue".into());
    m.insert("*\\Windows\\*".into(), "blue".into());
    m.insert("*\\Program Files*".into(), "blue".into());
    // 应用数据
    m.insert("*\\AppData\\*".into(), "gray".into());
    m.insert("*\\Local\\*".into(), "gray".into());
    m.insert("*\\Roaming\\*".into(), "gray".into());
    m
}

/// 合并两张 map（ls.yaml 覆盖内置默认）
fn merge_color_maps(defaults: HashMap<String, String>, yaml_overrides: HashMap<String, String>) -> HashMap<String, String> {
    let mut merged = defaults;
    for (k, v) in yaml_overrides {
        merged.insert(k, v);
    }
    merged
}

/// 获取环境变量配置（内置默认 + ls.yaml 覆盖）
pub fn get_variable_color_map() -> HashMap<String, String> {
    let cfg = load_ls_config();
    let yaml_colors = invert_map(&cfg.variables);
    let yaml_styles = invert_map(&cfg.variable_styles);
    let mut yaml_all = yaml_colors;
    for (k, v) in yaml_styles {
        yaml_all.entry(k).or_insert(v);
    }
    merge_color_maps(default_variable_colors(), yaml_all)
}

/// 获取排除的环境变量集合
pub fn get_exclude_set() -> Vec<String> {
    load_ls_config().exclude
}

/// 获取 PATH 路径配置（内置默认 + ls.yaml 覆盖）
pub fn get_path_color_map() -> HashMap<String, String> {
    let cfg = load_ls_config();
    let yaml_colors = invert_map(&cfg.paths);
    let yaml_styles = invert_map(&cfg.path_styles);
    let mut yaml_all = yaml_colors;
    for (k, v) in yaml_styles {
        yaml_all.entry(k).or_insert(v);
    }
    merge_color_maps(default_path_colors(), yaml_all)
}

/// 简单的通配符匹配，只支持 `*`（匹配任意字符）
pub fn wildmatch(pattern: &str, text: &str) -> bool {
    let pattern_lower = pattern.to_lowercase();
    let text_lower = text.to_lowercase();

    // 将通配符模式转为简单的逐段匹配
    let parts: Vec<&str> = pattern_lower.split('*').collect();
    if parts.len() == 1 {
        // 没有通配符
        return text_lower == pattern_lower;
    }

    let mut pos = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            // 第一个分段必须从开头匹配
            if !text_lower.starts_with(part) {
                return false;
            }
            pos = part.len();
        } else if i == parts.len() - 1 {
            // 最后一个分段必须匹配到结尾
            if !text_lower[pos..].ends_with(part) {
                return false;
            }
        } else {
            // 中间分段可以在任意位置
            match text_lower[pos..].find(part) {
                Some(idx) => pos += idx + part.len(),
                None => return false,
            }
        }
    }
    true
}
