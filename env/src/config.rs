use std::collections::HashMap;
use std::path::PathBuf;

/// e.yaml 顶层结构
#[derive(Debug, Default, serde::Deserialize)]
pub struct EConfig {
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

/// 内置默认环境变量配色（参照 ss.yaml）
fn default_variable_colors() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("SYSTEMDRIVE".into(), "gray".into());
    m.insert("SYSTEMROOT".into(), "gray".into());
    m.insert("TEMP".into(), "gray".into());
    m.insert("TMP".into(), "gray".into());
    m.insert("USERNAME".into(), "gray".into());
    m.insert("USERPROFILE".into(), "gray".into());
    m.insert("COMSPEC".into(), "gray".into());
    m.insert("PROMPT".into(), "gray".into());
    m.insert("CONDA_HOME".into(), "green".into());
    m.insert("MYSQL_HOME".into(), "lightblue".into());
    m.insert("NVM_HOME".into(), "lightcyan".into());
    m.insert("NVM_SYMLINK".into(), "lightcyan".into());
    m.insert("ENV".into(), "purple".into());
    m.insert("ENV_PATH".into(), "purple".into());
    m.insert("KMP_DUPLICATE_LIB_OK".into(), "purple".into());
    m.insert("REDIS_HOME".into(), "red".into());
    m.insert("PYTHON_HOME".into(), "lightyellow".into());
    m.insert("JAVA_HOME".into(), "blue".into());
    m.insert("GOPATH".into(), "blue".into());
    m.insert("SHADOWBOT_CULTURE".into(), "lightred".into());
    m.insert("SHADOWBOT_ROOT_X64".into(), "lightred".into());
    m.insert("RUSTUP_UPDATE_ROOT".into(), "lightpurple".into());
    m.insert("RUSTUP_DIST_SERVER".into(), "lightpurple".into());
    m
}

/// 内置默认 PATH 路径配色（参照 pp.yaml）
fn default_path_colors() -> HashMap<String, String> {
    let mut m = HashMap::new();
    // 系统
    m.insert("C:\\Windows*".into(), "gray".into());
    // 我的脚本
    m.insert("*\\Notepad_file\\*".into(), "lightgreen".into());
    // Git / WindowsApps
    m.insert("*\\Program Files\\Git\\cmd".into(), "yellow".into());
    m.insert("*\\AppData\\Local\\Microsoft\\WindowsApps".into(), "yellow".into());
    // Python
    m.insert("*\\Scripts".into(), "lightyellow".into());
    m.insert("*\\AppData\\Local\\Programs\\Python\\*".into(), "lightyellow".into());
    // Conda
    m.insert("*\\Anaconda3\\envs\\*".into(), "green".into());
    m.insert("*\\Anaconda3\\condabin".into(), "green".into());
    m.insert("*\\miniconda3\\condabin".into(), "green".into());
    // Nvm
    m.insert("*\\AppData\\Roaming\\nvm".into(), "lightcyan".into());
    m.insert("*\\AppData\\Local\\nvm".into(), "lightcyan".into());
    // Node
    m.insert("C:\\Program Files\\nodejs".into(), "cyan".into());
    m.insert("*\\nvm_nodejs".into(), "cyan".into());
    m.insert("*\\nvm\\*".into(), "cyan".into());
    // MySQL
    m.insert("*\\mysql-8.0.35-winx64\\bin".into(), "lightblue".into());
    // 影刀
    m.insert("*\\Program Files\\ShadowBot".into(), "lightred".into());
    m
}

/// 合并两张 map（yaml 覆盖内置默认）
fn merge_maps(defaults: HashMap<String, String>, overrides: HashMap<String, String>) -> HashMap<String, String> {
    let mut merged = defaults;
    for (k, v) in overrides {
        merged.insert(k, v);
    }
    merged
}

/// 获取配置目录路径（%LOCALAPPDATA%\e\）
pub fn get_config_dir() -> PathBuf {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(local).join("e")
}

/// 生成默认配置的 YAML 内容
fn generate_default_yaml() -> String {
    r#"# e 配色配置
# 首次运行时自动生成，可自由修改
# 格式: 颜色名: [变量名列表]

# e set 环境变量配色
variables:
  gray:
    - SYSTEMDRIVE
    - SYSTEMROOT
    - TEMP
    - TMP
    - USERNAME
    - USERPROFILE
    - COMSPEC
    - PROMPT
  green:
    - CONDA_HOME
  lightblue:
    - MYSQL_HOME
  lightcyan:
    - NVM_HOME
    - NVM_SYMLINK
  purple:
    - ENV
    - ENV_PATH
    - KMP_DUPLICATE_LIB_OK
  red:
    - REDIS_HOME
  lightyellow:
    - PYTHON_HOME
  blue:
    - JAVA_HOME
    - GOPATH
  lightred:
    - SHADOWBOT_CULTURE
    - SHADOWBOT_ROOT_X64
  lightpurple:
    - RUSTUP_UPDATE_ROOT
    - RUSTUP_DIST_SERVER

# e path PATH 路径配色
paths:
  gray:
    - "C:\\Windows*"
  lightgreen:
    - "*\\Notepad_file\\*"
  yellow:
    - "*\\Program Files\\Git\\cmd"
    - "*\\AppData\\Local\\Microsoft\\WindowsApps"
  lightyellow:
    - "*\\Scripts"
    - "*\\AppData\\Local\\Programs\\Python\\*"
  green:
    - "*\\Anaconda3\\envs\\*"
    - "*\\Anaconda3\\condabin"
    - "*\\miniconda3\\condabin"
  lightcyan:
    - "*\\AppData\\Roaming\\nvm"
    - "*\\AppData\\Local\\nvm"
  cyan:
    - "C:\\Program Files\\nodejs"
    - "*\\nvm_nodejs"
    - "*\\nvm\\*"
  lightblue:
    - "*\\mysql-8.0.35-winx64\\bin"
  lightred:
    - "*\\Program Files\\ShadowBot"
"#.to_string()
}

/// 获取配置文件路径
pub fn get_config_path() -> PathBuf {
    get_config_path_inner()
}

fn get_config_path_inner() -> PathBuf {
    get_config_dir().join("e.yaml")
}

/// 确保配置文件存在，不存在则创建默认配置
pub fn ensure_config() -> PathBuf {
    let path = get_config_path_inner();
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
    let path = get_config_path_inner();
    if path.exists() {
        let _ = std::fs::remove_file(&path);
        true
    } else {
        false
    }
}

/// 加载 e.yaml
fn load_config() -> EConfig {
    let path = get_config_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            serde_yaml::from_str(&content).unwrap_or_else(|e| {
                eprintln!("{} 配置文件 {} 解析失败: {}", color::yellow("注意:"), path.display(), e);
                eprintln!("{} 将使用默认配色", color::gray("提示:"));
                EConfig::default()
            })
        }
        Err(_) => {
            // 文件不存在，创建默认配置并告知用户
            let created = ensure_config();
            eprintln!("{} 已创建默认配置文件: {}", color::yellow("注意:"), created.display());
            // 重新读取
            std::fs::read_to_string(&created)
                .ok()
                .and_then(|c| serde_yaml::from_str(&c).ok())
                .unwrap_or_default()
        }
    }
}

/// 获取环境变量配色（默认 + yaml 覆盖）
pub fn get_variable_color_map() -> HashMap<String, String> {
    let cfg = load_config();
    let yaml_colors = invert_map(&cfg.variables);
    let yaml_styles = invert_map(&cfg.variable_styles);
    let mut yaml_all = yaml_colors;
    for (k, v) in yaml_styles {
        yaml_all.entry(k).or_insert(v);
    }
    merge_maps(default_variable_colors(), yaml_all)
}

/// 获取排除的环境变量
pub fn get_exclude_set() -> Vec<String> {
    load_config().exclude
}

/// 获取 PATH 路径配色（默认 + yaml 覆盖）
pub fn get_path_color_map() -> HashMap<String, String> {
    let cfg = load_config();
    let yaml_colors = invert_map(&cfg.paths);
    let yaml_styles = invert_map(&cfg.path_styles);
    let mut yaml_all = yaml_colors;
    for (k, v) in yaml_styles {
        yaml_all.entry(k).or_insert(v);
    }
    merge_maps(default_path_colors(), yaml_all)
}

/// 通配符匹配（仅支持 `*`，内部 `*` 为非贪心匹配）。
///
/// 注意：多个 `*` 时，中间的 `*` 以首次匹配为准（非贪心），
/// 例如 `*\\foo\\*` 匹配 `C:\\foo\\bar\\foo\\baz` 会返回 `true`（预期行为）。
pub fn wildmatch(pattern: &str, text: &str) -> bool {
    let pattern_lower = pattern.to_lowercase();
    let text_lower = text.to_lowercase();
    let parts: Vec<&str> = pattern_lower.split('*').collect();
    if parts.len() == 1 {
        return text_lower == pattern_lower;
    }
    let mut pos = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() { continue; }
        if i == 0 {
            if !text_lower.starts_with(part) { return false; }
            pos = part.len();
        } else if i == parts.len() - 1 {
            if !text_lower[pos..].ends_with(part) { return false; }
        } else {
            match text_lower[pos..].find(part) {
                Some(idx) => pos += idx + part.len(),
                None => return false,
            }
        }
    }
    true
}
