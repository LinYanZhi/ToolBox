use std::collections::HashMap;
use std::path::PathBuf;

use color::*;

/// 环境定义
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct EnvDef {
    /// PROMPT 格式（如 "[home] $P$G"）
    #[serde(default)]
    pub prompt: String,
    /// 要设置的环境变量
    #[serde(default)]
    pub variables: HashMap<String, String>,
    /// 要追加到 PATH 前的路径
    #[serde(default)]
    pub paths_prepend: Vec<String>,
}

/// 环境快照（用于内嵌到 deactivate 脚本中）
struct EnvSnapshot {
    old_path: String,
    old_prompt: String,
    old_variables: HashMap<String, String>,
    set_variables: Vec<String>,
}

// ── 目录 ──────────────────────────────────

/// 获取脚本输出目录：%LOCALAPPDATA%\e\
fn get_scripts_dir() -> PathBuf {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(local).join("e")
}

/// 获取环境定义目录
pub fn get_envs_dir() -> PathBuf {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(local).join("e").join("envs")
}

/// 列出所有可用环境名
pub fn list_envs() -> Vec<String> {
    let dir = get_envs_dir();
    if !dir.exists() {
        return Vec::new();
    }
    let mut envs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("yaml") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    envs.push(stem.to_string());
                }
            }
        }
    }
    envs.sort();
    envs
}

/// 加载指定环境的定义
pub fn load_env(name: &str) -> Option<EnvDef> {
    let dir = get_envs_dir();
    let path = dir.join(format!("{}.yaml", name));
    let content = std::fs::read_to_string(&path).ok()?;
    serde_yaml::from_str(&content).ok()
}

// ── 创建新环境 (e venv) ─────────────────────────

/// 创建一个新的环境定义文件，返回其路径
pub fn create_env(name: &str) -> Result<PathBuf, String> {
    let dir = get_envs_dir();
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("无法创建目录 {}: {}", dir.display(), e))?;
    }

    let path = dir.join(format!("{}.yaml", name));
    if path.exists() {
        return Err(format!("环境 '{}' 已存在: {}", name, path.display()));
    }

    let template = format!(
        r#"# 环境: {}
# 文件位置: {}
prompt: "[{}] $P$G"

# 要设置的环境变量
# variables:
#   MY_VAR: value

# 要追加到 PATH 前的目录
# paths_prepend:
#   - C:\path\to\add
"#,
        name, path.display(), name
    );

    std::fs::write(&path, template).map_err(|e| format!("无法写入 {}: {}", path.display(), e))?;
    Ok(path)
}

// ── 生成激活/停用脚本 ─────────────────────────

/// 生成激活脚本和停用脚本（停用脚本内嵌快照值）。
///
/// 输出到 %LOCALAPPDATA%\e\ 目录：
///   - activate-<name>.bat     (cmd)
///   - Activate-<name>.ps1     (PowerShell)
///   - deactivate-<name>.bat   (cmd)
///   - Deactivate-<name>.ps1   (PowerShell)
pub fn write_activate_scripts(def: &EnvDef, name: &str) -> Result<(), String> {
    let dir = get_scripts_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("无法创建目录 {}: {}", dir.display(), e))?;

    let snapshot = capture_snapshot(name, def);

    // ── cmd 激活脚本 ──
    write_activate_bat(&dir, name, def)?;
    // ── PowerShell 激活脚本 ──
    write_activate_ps1(&dir, name, def)?;
    // ── cmd 停用脚本（内嵌快照） ──
    write_deactivate_bat(&dir, name, &snapshot)?;
    // ── PowerShell 停用脚本（内嵌快照） ──
    write_deactivate_ps1(&dir, name, &snapshot)?;

    Ok(())
}

// ── CMD 激活脚本 ──

fn write_activate_bat(dir: &PathBuf, name: &str, def: &EnvDef) -> Result<(), String> {
    let path = dir.join(format!("activate-{}.bat", name));
    let mut lines = Vec::new();

    lines.push("@echo off".to_string());
    lines.push(format!("REM 激活环境: {}", name));
    lines.push(String::new());

    // 保存旧值
    lines.push("set \"_E_OLD_PATH=%PATH%\"".to_string());
    lines.push(format!("set \"_E_OLD_PROMPT=%PROMPT%\""));
    for key in def.variables.keys() {
        lines.push(format!("set \"_E_OLD_{}=%{}%\"", key, key));
    }
    lines.push(String::new());

    // 设置新变量
    for (key, val) in &def.variables {
        lines.push(format!("set \"{}={}\"", key, val));
    }

    // PATH 前置追加
    if !def.paths_prepend.is_empty() {
        for p in &def.paths_prepend {
            lines.push(format!("set \"PATH={};%PATH%\"", p));
        }
    }

    // PROMPT
    if !def.prompt.is_empty() {
        lines.push(format!("set \"PROMPT={}\"", def.prompt));
    }

    lines.push(String::new());
    lines.push(format!("echo {} {} @{}@",
        green("已激活环境:"),
        cyan(""),
        cyan(name)));
    lines.push(format!("echo {}",
        gray("运行 deactivate-{}.bat 恢复".to_string())));
    lines.push(format!("echo {}",
        gray(format!("停用: %LOCALAPPDATA%\\e\\deactivate-{}.bat", name))));

    let content = lines
        .iter()
        .map(|l| {
            // 替换 ANSI 标记
            l.replace("@", "")
        })
        .collect::<Vec<_>>()
        .join("\r\n");

    // 去掉 ANSI 转义，保留纯文本
    let content = strip_ansi_escapes(&content);

    std::fs::write(&path, content)
        .map_err(|e| format!("无法写入 {}: {}", path.display(), e))?;
    Ok(())
}

/// 去掉 ANSI 转义序列（批处理不支持）
fn strip_ansi_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // 跳过 CSI 序列：\x1b[...m
            while let Some(n) = chars.next() {
                if n == 'm' { break; }
            }
        } else {
            out.push(c);
        }
    }
    out
}

// ── PowerShell 激活脚本 ──

fn write_activate_ps1(dir: &PathBuf, name: &str, def: &EnvDef) -> Result<(), String> {
    let path = dir.join(format!("Activate-{}.ps1", name));
    let mut lines = Vec::new();

    lines.push(format!("# 激活环境: {}", name));
    lines.push(String::new());

    // 保存旧值
    lines.push("$_E_OLD_PATH = $env:PATH".to_string());
    lines.push("$_E_OLD_PROMPT = $env:PROMPT".to_string());
    for key in def.variables.keys() {
        lines.push(format!("$_E_OLD_{0} = $env:{0}", key));
    }
    lines.push(String::new());

    // 设置新变量
    for (key, val) in &def.variables {
        lines.push(format!("$env:{} = '{}'", key, val));
    }

    // PATH 前置追加
    if !def.paths_prepend.is_empty() {
        for p in &def.paths_prepend {
            lines.push(format!("$env:PATH = '{};' + $env:PATH", p));
        }
    }

    // PROMPT
    if !def.prompt.is_empty() {
        lines.push(format!("$env:PROMPT = '{}'", def.prompt));
    }

    lines.push(String::new());
    lines.push(format!("Write-Host \"已激活环境: {}\" -ForegroundColor Green", name));
    lines.push(format!("Write-Host \"停用: . '{}\" -ForegroundColor Gray",
        dir.join(format!("Deactivate-{}.ps1", name)).to_string_lossy()));

    let content = lines.join("\r\n");
    std::fs::write(&path, content)
        .map_err(|e| format!("无法写入 {}: {}", path.display(), e))?;
    Ok(())
}

// ── CMD 停用脚本（内嵌快照值） ──

fn write_deactivate_bat(dir: &PathBuf, name: &str, snap: &EnvSnapshot) -> Result<(), String> {
    let path = dir.join(format!("deactivate-{}.bat", name));
    let mut lines = Vec::new();

    lines.push("@echo off".to_string());
    lines.push(format!("REM 停用环境: {}", name));
    lines.push(String::new());

    // 恢复 PATH（直接从脚本变量恢复，不依赖 _E_OLD_PATH 保存的值）
    // 但优先使用 _E_OLD_PATH（如果用户没手动改动过）
    lines.push(format!("if defined _E_OLD_PATH (set \"PATH=%_E_OLD_PATH%\") else (set \"PATH={}\")",
        snap.old_path));

    // 恢复 PROMPT
    if !snap.old_prompt.is_empty() {
        lines.push(format!("if defined _E_OLD_PROMPT (set \"PROMPT=%_E_OLD_PROMPT%\") else (set \"PROMPT={}\")",
            snap.old_prompt));
    } else {
        lines.push("set \"PROMPT=%_E_OLD_PROMPT%\"".to_string());
    }

    // 恢复变量
    for key in &snap.set_variables {
        if let Some(old_val) = snap.old_variables.get(key) {
            lines.push(format!(
                "if defined _E_OLD_{key} (set \"{key}=%_E_OLD_{key}%\") else (set \"{key}={val}\")",
                key = key, val = old_val));
        } else {
            lines.push(format!("set \"{}=\"", key));
        }
    }

    // 清理 _E_OLD_ 变量
    lines.push("set \"_E_OLD_PATH=\"".to_string());
    lines.push("set \"_E_OLD_PROMPT=\"".to_string());
    for key in &snap.set_variables {
        lines.push(format!("set \"_E_OLD_{}=\"", key));
    }

    lines.push(String::new());
    lines.push(format!("echo 已退出环境: {}", name));

    let content = strip_ansi_escapes(&lines.join("\r\n"));
    std::fs::write(&path, content)
        .map_err(|e| format!("无法写入 {}: {}", path.display(), e))?;
    Ok(())
}

// ── PowerShell 停用脚本（内嵌快照值） ──

fn write_deactivate_ps1(dir: &PathBuf, name: &str, snap: &EnvSnapshot) -> Result<(), String> {
    let path = dir.join(format!("Deactivate-{}.ps1", name));
    let mut lines = Vec::new();

    lines.push(format!("# 停用环境: {}", name));
    lines.push(String::new());

    // 恢复 PATH
    lines.push(format!(
        "if ($_E_OLD_PATH -ne $null) {{ $env:PATH = $_E_OLD_PATH }} else {{ $env:PATH = '{}' }}",
        snap.old_path));

    // 恢复 PROMPT
    if !snap.old_prompt.is_empty() {
        lines.push(format!(
            "if ($_E_OLD_PROMPT -ne $null) {{ $env:PROMPT = $_E_OLD_PROMPT }} else {{ $env:PROMPT = '{}' }}",
            snap.old_prompt));
    }

    // 恢复变量
    for key in &snap.set_variables {
        if let Some(old_val) = snap.old_variables.get(key) {
            lines.push(format!(
                "if ($_E_OLD_{key} -ne $null) {{ $env:{key} = $_E_OLD_{key} }} else {{ $env:{key} = '{val}' }}",
                key = key, val = old_val));
        } else {
            lines.push(format!("Remove-Item Env:{} -ErrorAction SilentlyContinue", key));
        }
    }

    lines.push(String::new());
    lines.push(format!("Write-Host \"已退出环境: {}\" -ForegroundColor Green", name));

    let content = lines.join("\r\n");
    std::fs::write(&path, content)
        .map_err(|e| format!("无法写入 {}: {}", path.display(), e))?;
    Ok(())
}

// ── 快照 ──────────────────────────────────

fn capture_snapshot(_env_name: &str, def: &EnvDef) -> EnvSnapshot {
    let old_path = std::env::var("PATH").unwrap_or_default();
    let old_prompt = std::env::var("PROMPT").unwrap_or_default();
    let mut old_variables = HashMap::new();
    for key in def.variables.keys() {
        if let Ok(val) = std::env::var(key) {
            old_variables.insert(key.clone(), val);
        }
    }
    EnvSnapshot {
        old_path,
        old_prompt,
        old_variables,
        set_variables: def.variables.keys().cloned().collect(),
    }
}

// ── 打印环境列表（终端展示） ─────────────────────

pub fn print_env_list() {
    let envs_dir = get_envs_dir();
    println!("  {}", bold_cyan("可用环境:"));
    println!();

    let envs = list_envs();
    if envs.is_empty() {
        println!("  {} 在 {} 下没有找到环境定义", gray("•"), gray(envs_dir.display().to_string()));
        println!("  {} 使用 e venv <环境名> 创建一个", gray("•"));
        println!("  {} 使用 e config 查看配置目录", gray("•"));
        println!();
        println!("  {}", gray(format!("位置: {}", envs_dir.display())));
        return;
    }

    let scripts_dir = get_scripts_dir();
    for name in &envs {
        if let Some(def) = load_env(name) {
            let var_count = def.variables.len();
            let path_count = def.paths_prepend.len();
            let desc = if !def.prompt.is_empty() {
                format!("PROMPT: {}, 变量: {}, PATH: {}", def.prompt, var_count, path_count)
            } else {
                format!("变量: {}, PATH: {}", var_count, path_count)
            };
            println!("  {}    {}",
                pad_left(&cyan(name), envs.iter().map(|n| n.display_width()).max().unwrap_or(10)),
                gray(&desc));
        }
    }
    println!();
    println!("  {}", bold_yellow("使用方法:"));
    println!();
    for name in &envs {
        let bat = scripts_dir.join(format!("activate-{}.bat", name));
        let ps1 = scripts_dir.join(format!("Activate-{}.ps1", name));
        println!("  {}", cyan(name));
        println!("    {} {}", green("cmd:"),        gray(bat.to_string_lossy()));
        println!("    {} {}", green("PowerShell:"), gray(ps1.to_string_lossy()));
    }
    println!();
    println!("  {}", gray("先运行 e activate <环境名> 生成脚本，然后执行对应脚本激活。"));
    println!();
    println!("  {}", gray(format!("定义文件: {}", envs_dir.display())));
    println!("  {}", gray(format!("脚本目录: {}", scripts_dir.display())));
}
