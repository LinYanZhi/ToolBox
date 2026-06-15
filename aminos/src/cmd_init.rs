use std::fs;

use anyhow::{Context, bail};
use color;
use crate::{paths, cmd_names};

/// 初始化 as 环境：创建 tools/bin 目录并注册到 PATH。
pub fn run_init() -> anyhow::Result<()> {
    let bin_dir = paths::tools_bin_dir();
    fs::create_dir_all(&bin_dir)?;
    println!("✓ 已创建: {}", bin_dir.display());

    let bin_path = bin_dir.to_string_lossy().to_string();

    // 通过 PowerShell 读取注册表中的用户 PATH（不包含当前会话的临时修改）
    let ps_get_path = format!(
        "[Environment]::GetEnvironmentVariable('PATH', 'User')"
    );
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_get_path])
        .output()
        .context("读取注册表 PATH 失败")?;
    let user_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

    let already_in_path = user_path.split(';')
        .any(|p| p.trim().to_lowercase() == bin_path.to_lowercase());

    if already_in_path {
        println!("✓ {} 已在用户 PATH 中", bin_dir.display());
        return Ok(());
    }

    // 将新路径追加到用户 PATH（不携带当前会话临时修改）
    let new_path = if user_path.is_empty() {
        bin_path.clone()
    } else {
        format!("{};{}", user_path, bin_path)
    };

    let ps_set_path = format!(
        "[Environment]::SetEnvironmentVariable('PATH', '{}', 'User')",
        new_path.replace('\'', "''")
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_set_path])
        .status()
        .context("注册 PATH 失败")?;

    if status.success() {
        let is_powershell = detect_powershell();
        println!("✓ 已将 {} 添加到用户 PATH", bin_dir.display());
        println!("  新打开的终端将自动生效。");
        println!("  如需在当前终端立即生效，请执行:");
        println!();
        if is_powershell {
            println!("    {}", color::bold_green(&format!("$env:PATH = \"{};$env:PATH\"", bin_path)));
        } else {
            println!("    {}", color::bold_green(&format!("set PATH={};%PATH%", bin_path)));
        }
        println!();
    } else {
        bail!("添加 PATH 失败");
    }

    println!("\n✓ as 环境初始化完成");
    println!("  现在可通过 {} 安装自研工具", color::cyan(cmd_names::INSTALL));
    Ok(())
}

/// 检测当前终端是否为 PowerShell（而非 cmd.exe）
fn detect_powershell() -> bool {
    if let Some(ppid) = get_parent_process_name() {
        let lower = ppid.to_lowercase();
        lower.contains("powershell") || lower.contains("pwsh")
    } else {
        false
    }
}

/// 获取父进程名称
fn get_parent_process_name() -> Option<String> {
    #[cfg(windows)]
    {
        use std::process::Command;
        // 通过 wmic 获取父进程名称
        let pid = std::process::id();
        let output = Command::new("wmic")
            .args(["process", "where", &format!("ProcessId={}", pid), "get", "ParentProcessId"])
            .output()
            .ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let parent_pid = stdout
            .lines()
            .nth(1)
            .and_then(|l| l.trim().parse::<u32>().ok())?;

        let output = Command::new("wmic")
            .args(["process", "where", &format!("ProcessId={}", parent_pid), "get", "Name"])
            .output()
            .ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let name = stdout.lines().nth(1).map(|l| l.trim().to_string())?;
        Some(name)
    }
    #[cfg(not(windows))]
    {
        None
    }
}
