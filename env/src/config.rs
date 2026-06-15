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

/// 内置默认环境变量配色
fn default_variable_colors() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("PATH".into(), "cyan".into());
    m.insert("PWD".into(), "cyan".into());
    m.insert("OLDPWD".into(), "cyan".into());
    m.insert("HOME".into(), "cyan".into());
    m.insert("USERPROFILE".into(), "cyan".into());
    m.insert("USERNAME".into(), "green".into());
    m.insert("USER".into(), "green".into());
    m.insert("COMPUTERNAME".into(), "green".into());
    m.insert("HOSTNAME".into(), "green".into());
    m.insert("SystemRoot".into(), "blue".into());
    m.insert("WINDIR".into(), "blue".into());
    m.insert("ProgramFiles".into(), "blue".into());
    m.insert("ProgramFiles(x86)".into(), "blue".into());
    m.insert("ProgramData".into(), "blue".into());
    m.insert("ALLUSERSPROFILE".into(), "blue".into());
    m.insert("TEMP".into(), "yellow".into());
    m.insert("TMP".into(), "yellow".into());
    m.insert("JAVA_HOME".into(), "red".into());
    m.insert("GOROOT".into(), "red".into());
    m.insert("RUSTUP_HOME".into(), "red".into());
    m.insert("CARGO_HOME".into(), "red".into());
    m.insert("SHELL".into(), "purple".into());
    m.insert("EDITOR".into(), "purple".into());
    m.insert("VISUAL".into(), "purple".into());
    m.insert("TERM".into(), "lightblue".into());
    m
}

/// 内置默认 PATH 路径配色
fn default_path_colors() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("*aminos*".into(), "cyan".into());
    m.insert("*\\.cargo\\*".into(), "cyan".into());
    m.insert("*\\.tool\\*".into(), "cyan".into());
    m.insert("*\\Python\\*".into(), "green".into());
    m.insert("*\\nodejs\\*".into(), "green".into());
    m.insert("*\\Git\\*".into(), "lightred".into());
    m.insert("*\\Java\\*".into(), "red".into());
    m.insert("*\\Go\\*".into(), "red".into());
    m.insert("*\\System32\\*".into(), "blue".into());
    m.insert("*\\Windows\\*".into(), "blue".into());
    m.insert("*\\Program Files*".into(), "blue".into());
    m.insert("*\\AppData\\*".into(), "gray".into());
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

/// 获取 e.yaml 路径（exe 同级）
fn get_config_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        exe.parent().unwrap_or(std::path::Path::new(".")).join("e.yaml")
    } else {
        PathBuf::from("e.yaml")
    }
}

/// 加载 e.yaml
fn load_config() -> EConfig {
    let path = get_config_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_yaml::from_str(&content).unwrap_or_default(),
        Err(_) => EConfig::default(),
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

/// 通配符匹配（仅支持 *）
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
