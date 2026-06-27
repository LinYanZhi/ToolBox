use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context};

use crate::paths;

/// 执行 PowerShell 命令并返回 stdout。
pub(crate) fn ps_exec(command: &str) -> anyhow::Result<String> {
    let mut child = Command::new("powershell")
        .args(["-NoProfile", "-Command", command])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("启动 PowerShell 失败")?;

    // 30 秒超时，防止 PowerShell 挂死
    const PS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
    let now = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output().unwrap_or_else(|_| {
                    std::process::Output {
                        status,
                        stdout: Vec::new(),
                        stderr: Vec::new(),
                    }
                });
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    bail!("PowerShell 错误: {}", stderr.trim());
                }
                return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
            }
            Ok(None) => {
                if now.elapsed() > PS_TIMEOUT {
                    let _ = child.kill();
                    bail!("PowerShell 执行超时（{} 秒）", PS_TIMEOUT.as_secs());
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                bail!("PowerShell 进程错误: {}", e);
            }
        }
    }
}

/// 获取快捷方式的目标路径。
pub(crate) fn get_shortcut_target(lnk_path: &str) -> Option<String> {
    let cmd = format!(
        "$ws = New-Object -ComObject WScript.Shell; \
         $sc = $ws.CreateShortcut('{}'); \
         $sc.TargetPath",
        lnk_path.replace('\'', "''")
    );
    ps_exec(&cmd).ok()
}

/// 复制已有快捷方式到指定位置（仅复制目标目录，重新创建指向该目录的快捷方式）。
pub(crate) fn create_shortcut_file(source_lnk: &str, output_lnk: &Path) -> anyhow::Result<()> {
    let target = get_shortcut_target(source_lnk).unwrap_or_default();
    let dir = Path::new(&target).parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    create_shortcut_dir(&dir, output_lnk)
}

/// 创建指向目录的快捷方式（在 apps/ 目录中）。
pub(crate) fn create_shortcut_dir(target_dir: &str, output_lnk: &Path) -> anyhow::Result<()> {
    let out_dir = output_lnk.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(out_dir)?;

    let cmd = format!(
        "$ws = New-Object -ComObject WScript.Shell; \
         $sc = $ws.CreateShortcut('{}'); \
         $sc.TargetPath = '{}'; \
         $sc.Save()",
        output_lnk.to_string_lossy().replace('\'', "''"),
        target_dir.replace('\'', "''"),
    );
    ps_exec(&cmd)?;
    Ok(())
}

/// 创建指向可执行文件的快捷方式（含工作目录）。
pub(crate) fn create_shortcut_exe(target_exe: &str, output_lnk: &Path) -> anyhow::Result<()> {
    let out_dir = output_lnk.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(out_dir)?;

    let work_dir = Path::new(target_exe).parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let cmd = format!(
        "$ws = New-Object -ComObject WScript.Shell; \
         $sc = $ws.CreateShortcut('{}'); \
         $sc.TargetPath = '{}'; \
         $sc.WorkingDirectory = '{}'; \
         $sc.Save()",
        output_lnk.to_string_lossy().replace('\'', "''"),
        target_exe.replace('\'', "''"),
        work_dir.replace('\'', "''"),
    );
    ps_exec(&cmd)?;
    Ok(())
}

/// 展开 `%VAR%` 环境变量引用。
pub(crate) fn expand_env_vars(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let mut var = String::new();
            while let Some(&next) = chars.peek() {
                if next == '%' {
                    chars.next();
                    break;
                }
                var.push(next);
                chars.next();
            }
            if let Ok(val) = std::env::var(&var) {
                result.push_str(&val);
            } else {
                result.push('%');
                result.push_str(&var);
                result.push('%');
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Sort version strings in descending order (newest first).
///
/// 正确处理 pre-release 标签（如 "1.0.0-beta", "1.0.0-rc1"）：
///   1. 比较点分数字部分（如 1.0.0）
///   2. 数字部分相同时，正式版 > 预发布版（无 `-` 后缀 > 有 `-` 后缀）
///   3. 均为预发布版时，按标签字符串降序（rc2 > rc1, beta > alpha）
pub(crate) fn sort_versions_desc(versions: &mut [&str]) {
    versions.sort_by(|a, b| {
        // 拆分为基础版本和 pre-release 后缀
        let a_pre = a.splitn(2, '-').collect::<Vec<_>>();
        let b_pre = b.splitn(2, '-').collect::<Vec<_>>();
        let a_base = a_pre[0];
        let b_base = b_pre[0];

        let a_segs: Vec<u32> = a_base.split('.').filter_map(|s| s.parse().ok()).collect();
        let b_segs: Vec<u32> = b_base.split('.').filter_map(|s| s.parse().ok()).collect();

        for i in 0..a_segs.len().max(b_segs.len()) {
            let av = a_segs.get(i).copied().unwrap_or(0);
            let bv = b_segs.get(i).copied().unwrap_or(0);
            match bv.cmp(&av) {
                std::cmp::Ordering::Equal => continue,
                other => return other,
            }
        }

        // 数字部分相同 → 比较 pre-release 后缀
        match (a_pre.get(1), b_pre.get(1)) {
            (None, None) => b.cmp(a),
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            (Some(pa), Some(pb)) => pb.cmp(pa),
        }
    });
}

/// 检查 tools/bin 是否已注册到用户 PATH（读取注册表，不受当前会话影响）
pub(crate) fn is_tools_bin_in_user_path() -> bool {
    let bin_dir = paths::tools_bin_dir();
    let bin_path = bin_dir.to_string_lossy().to_string();

    let ps_cmd = format!("[Environment]::GetEnvironmentVariable('PATH', 'User')");
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_cmd])
        .output();
    match output {
        Ok(o) => {
            let user_path = String::from_utf8_lossy(&o.stdout).trim().to_string();
            user_path.split(';')
                .any(|p| p.trim().to_lowercase() == bin_path.to_lowercase())
        }
        Err(_) => false,
    }
}

/// 检查 tools/bin 是否在当前终端会话的 PATH 中
pub(crate) fn is_tools_bin_in_session_path(bin_dir: &Path) -> bool {
    std::env::var("PATH").ok().map_or(false, |p| {
        let bin = bin_dir.to_string_lossy().to_lowercase();
        p.split(';').any(|s| s.trim().to_lowercase() == bin)
    })
}

/// 检测当前终端是否为 PowerShell（而非 cmd.exe）
pub(crate) fn detect_powershell() -> bool {
    if let Some(ppid) = get_parent_process_name() {
        let lower = ppid.to_lowercase();
        lower.contains("powershell") || lower.contains("pwsh")
    } else {
        false
    }
}

/// 获取父进程名称
pub(crate) fn get_parent_process_name() -> Option<String> {
    #[cfg(windows)]
    {
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
