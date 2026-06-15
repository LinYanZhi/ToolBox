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

/// 环境快照（保存当前会话环境，用于 deactivate 恢复）
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct EnvSnapshot {
    pub old_path: String,
    pub old_prompt: String,
    pub old_variables: HashMap<String, String>,
    pub set_variables: Vec<String>,
    pub env_name: String,
}

// ── 获取父进程 PID（Windows） ──────────────────

#[repr(C)]
struct ProcessBasicInfo {
    exit_status: i32,
    peb_base: *mut u8,
    affinity_mask: usize,
    base_priority: i32,
    unique_pid: usize,
    inherited_from: usize, // 父进程 PID
}

unsafe extern "system" {
    fn NtQueryInformationProcess(
        handle: isize,
        info_class: u32,
        info: *mut u8,
        len: u32,
        ret_len: *mut u32,
    ) -> i32;
    fn GetCurrentProcess() -> isize;
}

/// 获取调用者的父进程 PID（即 shell 的 PID）
fn get_session_id() -> u32 {
    unsafe {
        let mut pbi: ProcessBasicInfo = std::mem::zeroed();
        let status = NtQueryInformationProcess(
            GetCurrentProcess(),
            0, // ProcessBasicInformation
            &mut pbi as *mut _ as *mut u8,
            std::mem::size_of::<ProcessBasicInfo>() as u32,
            std::ptr::null_mut(),
        );
        if status == 0 {
            pbi.inherited_from as u32
        } else {
            // 失败回退到当前 PID
            std::process::id()
        }
    }
}

// ── 环境定义目录 ──────────────────────────────────

/// 获取环境定义目录（统一在 %LOCALAPPDATA%\e\envs\）
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

// ── 状态文件（按会话独立） ──────────────────

/// 状态文件路径：%TEMP%\e-state-<父进程PID>.json
fn get_state_path() -> PathBuf {
    let tmp = std::env::var("TEMP").unwrap_or_else(|_| ".".into());
    let session = get_session_id();
    PathBuf::from(tmp).join(format!("e-state-{}.json", session))
}

/// 保存当前环境快照
pub fn save_snapshot(snapshot: &EnvSnapshot) {
    if let Ok(json) = serde_json::to_string_pretty(snapshot) {
        let _ = std::fs::write(get_state_path(), json);
    }
}

/// 读取环境快照
pub fn load_snapshot() -> Option<EnvSnapshot> {
    let content = std::fs::read_to_string(get_state_path()).ok()?;
    serde_json::from_str(&content).ok()
}

/// 清除快照
pub fn clear_snapshot() {
    let _ = std::fs::remove_file(get_state_path());
}

// ── 输出激活脚本 ──────────────────────────────────

/// 生成 cmd.exe 激活脚本
pub fn print_activate_cmd(def: &EnvDef, env_name: &str) {
    let snapshot = capture_snapshot(env_name, def);
    save_snapshot(&snapshot);

    println!("@echo off");
    println!("REM e: activate environment '{}'", env_name);
    println!();

    for (key, val) in &def.variables {
        println!("@set \"{}={}\"", key, val);
    }

    if !def.paths_prepend.is_empty() {
        for p in &def.paths_prepend {
            println!("@set \"PATH={};%PATH%\"", p);
        }
    }

    if !def.prompt.is_empty() {
        println!("@set \"PROMPT={}\"", def.prompt);
    }

    println!();
    println!("@echo {} {}[{}]{} {}",
        green("激活环境:"),
        cyan(""),
        cyan(env_name),
        gray(""),
        gray("使用 e deactivate 恢复"));
}

/// 生成 PowerShell 激活脚本
pub fn print_activate_ps1(def: &EnvDef, env_name: &str) {
    let snapshot = capture_snapshot(env_name, def);
    save_snapshot(&snapshot);

    println!("# e: activate environment '{}'", env_name);
    println!();

    for (key, val) in &def.variables {
        println!("$env:{} = '{}'", key, val);
    }

    if !def.paths_prepend.is_empty() {
        for p in &def.paths_prepend {
            println!("$env:PATH = '{};' + $env:PATH", p);
        }
    }

    if !def.prompt.is_empty() {
        println!("$env:PROMPT = '{}'", def.prompt);
    }

    println!("Write-Host \"激活环境:\" -ForegroundColor Green");
    println!("Write-Host \" {}\" -NoNewline -ForegroundColor Cyan", env_name);
    println!("Write-Host \" - 使用 e deactivate 恢复\"");
}

fn capture_snapshot(env_name: &str, def: &EnvDef) -> EnvSnapshot {
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
        env_name: env_name.to_string(),
    }
}

// ── 输出停用脚本 ──────────────────────────────────

/// 生成 cmd.exe 停用脚本
pub fn print_deactivate_cmd() {
    let snapshot = match load_snapshot() {
        Some(s) => s,
        None => {
            eprintln!("{} 没有已激活的环境", red("错误:"));
            eprintln!("{} 请先运行 e activate <环境名>", gray("提示:"));
            return;
        }
    };

    println!("@echo off");
    println!("REM e: deactivate environment '{}'", snapshot.env_name);
    println!();

    println!("@set \"PATH={}\"", snapshot.old_path);

    if !snapshot.old_prompt.is_empty() {
        println!("@set \"PROMPT={}\"", snapshot.old_prompt);
    }

    for key in &snapshot.set_variables {
        if let Some(old_val) = snapshot.old_variables.get(key) {
            println!("@set \"{}={}\"", key, old_val);
        } else {
            println!("@set \"{}=\"", key);
        }
    }

    println!();
    println!("@echo {} {}[{}]{}",
        green("已退出环境:"),
        cyan(""),
        cyan(&snapshot.env_name),
        gray(""));

    clear_snapshot();
}

/// 生成 PowerShell 停用脚本
pub fn print_deactivate_ps1() {
    let snapshot = match load_snapshot() {
        Some(s) => s,
        None => {
            eprintln!("{} 没有已激活的环境", red("错误:"));
            eprintln!("{} 请先运行 e activate <环境名>", gray("提示:"));
            return;
        }
    };

    println!("# e: deactivate environment '{}'", snapshot.env_name);
    println!();

    println!("$env:PATH = '{}'", snapshot.old_path);

    if !snapshot.old_prompt.is_empty() {
        println!("$env:PROMPT = '{}'", snapshot.old_prompt);
    }

    for key in &snapshot.set_variables {
        if let Some(old_val) = snapshot.old_variables.get(key) {
            println!("$env:{} = '{}'", key, old_val);
        } else {
            println!("Remove-Item Env:{} -ErrorAction SilentlyContinue", key);
        }
    }

    println!("Write-Host \"已退出环境:\" -ForegroundColor Green");
    println!("Write-Host \" {}\" -ForegroundColor Cyan", snapshot.env_name);

    clear_snapshot();
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
    println!("  {}  {}  {}  {}",
        gray("激活:"),
        cyan("e activate <环境名> --cmd | iex"),
        gray(""),
        gray("(PowerShell)"));
    println!("  {}  {}  {}",
        gray("停用:"),
        cyan("e deactivate --cmd | iex"),
        gray("(PowerShell)"));
    println!();
    println!("  {}", gray(format!("定义文件: {}\\<环境名>.yaml", envs_dir.display())));
    println!("  {}", gray(format!("状态快照: %TEMP%\\e-state-<PID>.json（每个会话独立）")));
}
