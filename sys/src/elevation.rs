use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

/// 通过 PowerShell `Start-Process -Verb RunAs` 以管理员权限运行安装程序。
///
/// 这是 Windows UAC 提权的标准方法，会弹出 UAC 确认对话框。
pub fn try_elevate(installer_path: &Path, args: &[String]) -> Result<bool> {
    let mut ps_args = format!(
        "Start-Process -FilePath '{}'",
        installer_path.display()
    );
    if !args.is_empty() {
        let arg_str = args
            .iter()
            .map(|a| format!("'{}'", a.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(", ");
        ps_args.push_str(&format!(" -ArgumentList {}", arg_str));
    }
    ps_args.push_str(" -Verb RunAs -Wait");

    let status = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_args])
        .status()
        .context("运行 UAC 提权 PowerShell 命令失败")?;

    if !status.success() {
        anyhow::bail!("UAC 提权被取消或失败");
    }
    Ok(true)
}
