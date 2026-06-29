use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::bail;

use crate::paths;

use super::detect::detect_installer_type;

/// 安装器进程的最大等待时间（秒）。
/// 超时后若检测到软件已安装，则视为成功并强制结束进程。
const INSTALLER_TIMEOUT_SECS: u64 = 180;

/// 临时目录守卫：离开作用域时自动清理解压出来的安装器暂存目录。
struct StagingGuard(Option<PathBuf>);

impl StagingGuard {
    fn new(path: Option<PathBuf>) -> Self {
        Self(path)
    }
}

impl Drop for StagingGuard {
    fn drop(&mut self) {
        if let Some(ref path) = self.0 {
            let _ = fs::remove_dir_all(path);
        }
    }
}

/// 使用魔数检测文件是否为压缩包（zip/7z/rar 等）。
fn is_archive_file(path: &Path) -> bool {
    match net::detect_format(path) {
        Some(fmt) => !matches!(fmt, net::FileFormat::Pe),
        None => false,
    }
}

/// 在解压后的目录中查找安装器 exe 或 msi。
/// 优先匹配：setup.exe/msi → install.exe/msi → 含软件名的 exe/msi → 第一个 exe → 第一个 msi
fn find_installer_exe_in_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    let mut exe_candidates: Vec<PathBuf> = Vec::new();
    let mut msi_candidates: Vec<PathBuf> = Vec::new();
    let ext_match = |p: &Path, ext: &str| -> bool {
        p.extension().map_or(false, |e| e.eq_ignore_ascii_case(ext)) && p.is_file()
    };

    let mut scan = |d: &Path| -> Option<PathBuf> {
        let entries = fs::read_dir(d).ok()?;
        for entry in entries.flatten() {
            let p = entry.path();
            if ext_match(&p, "exe") || ext_match(&p, "msi") {
                let fname = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                if fname == "setup" || fname == "install" {
                    return Some(p);
                }
                if ext_match(&p, "exe") {
                    exe_candidates.push(p);
                } else {
                    msi_candidates.push(p);
                }
            }
        }
        None
    };

    // 先扫描根目录
    if let Some(found) = scan(dir) {
        return Some(found);
    }
    // 再扫描一级子目录
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if let Some(found) = scan(&entry.path()) {
                    return Some(found);
                }
            }
        }
    }

    // 按名称相似度排序
    let lower_name = name.to_lowercase();
    let score = |p: &PathBuf| -> usize {
        let fn_ = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        if fn_.contains(&lower_name) || lower_name.contains(&fn_) { 1 } else { 0 }
    };

    // exe 优先于 msi
    exe_candidates.sort_by(|a, b| score(b).cmp(&score(a)));
    msi_candidates.sort_by(|a, b| score(b).cmp(&score(a)));

    exe_candidates.into_iter().next()
        .or_else(|| msi_candidates.into_iter().next())
}

/// 执行安装器。
///
/// 便携版：直接解压，返回解压后的路径。
/// 安装版：运行安装器（静默或 GUI），不返回路径。
///
/// 特殊处理：如果下载文件是压缩包（如 nvm-setup.zip 内含 exe），
/// 会自动解压并找到内部的安装器 exe 来运行。
///
/// ## 核心理念：不等待进程，而是轮询检测安装状态
///
/// 传统方式 `cmd.wait()` 依赖进程退出，但很多 Windows 安装器
///（NSIS/Inno）装完软件后不会立即退出（启动了后台服务、主程序等），
/// 导致 as 卡死。
///
/// 新方式：启动安装器后，立即进入检测轮询循环，依次检查：
///   1. 注册表条目（detection.display_name + publisher）— 最可靠
///   2. 快捷方式文件是否出现（shortcut_candidates）
///   3. 安装目录是否存在（install_dir_candidates）
///
/// 检测到安装即返回成功，进程若仍存活则自动清理。
pub(crate) fn run_installer(
    name: &str,
    version: &str,
    installer_path: &Path,
    vi: &crate::software::VersionInfo,
    gui: bool,
) -> anyhow::Result<(bool, Option<PathBuf>)> {
    let itype = if vi.installer_type.is_empty() {
        detect_installer_type(installer_path)
    } else {
        &vi.installer_type
    };

    // Portable mode: extract archive
    if itype == "portable" {
        let path = install_portable(name, version, installer_path)?;
        return Ok((true, Some(path)));
    }

    // 安装版但文件是压缩包（如 nvm-setup.zip 内含 exe）→ 自动解压并找安装器
    let staging_path = if is_archive_file(installer_path) {
        let staging = paths::downloads_dir().join(format!("{}-{}-extracted", name, version));
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        fs::create_dir_all(&staging)?;
        extract_archive_to(installer_path, &staging)?;
        Some(staging)
    } else {
        None
    };
    let _staging_guard = StagingGuard::new(staging_path.clone());

    let actual_installer = if let Some(ref staging) = staging_path {
        let exe = find_installer_exe_in_dir(staging, name)
            .ok_or_else(|| anyhow::anyhow!("在解压目录中未找到安装器 exe"))?;
        println!("  找到安装器: {}", exe.display());
        exe
    } else {
        installer_path.to_path_buf()
    };

    // Build command（msi 需通过 msiexec /i 运行）
    let is_msi = actual_installer.extension().map_or(false, |e| e.eq_ignore_ascii_case("msi"));
    let mut cmd = if is_msi {
        let mut c = Command::new("msiexec");
        c.arg("/i");
        c.arg(&actual_installer);
        if !gui {
            c.arg("/qn");
        }
        c
    } else {
        Command::new(&actual_installer)
    };
    if !gui && !is_msi {
        for arg in &vi.install_args {
            cmd.arg(arg);
        }
    }

    if gui {
        println!("  以交互界面模式启动安装器");
    } else {
        println!("  静默安装 {} ...", itype);
    }

    // Spawn installer（不 wait，立即进入检测轮询）
    let mut child = match cmd.spawn() {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("  启动安装程序失败: {}", e);
            return Ok((false, None));
        }
    };

    let has_detection = vi.detection.is_some()
        || !vi.shortcut_candidates.is_empty()
        || !vi.install_dir_candidates.is_empty();

    if !has_detection {
        // 没有任何检测依据 → 退回到传统 wait
        println!("  未配置检测规则，等待安装器进程退出...");
        return fallback_wait_process(&mut child, &actual_installer, vi);
    }

    println!("  等待安装完成...");

    let timeout = Duration::from_secs(INSTALLER_TIMEOUT_SECS);
    let start = Instant::now();
    let mut printed_dot = false;

    loop {
        // 1. 检测安装状态（注册表/快捷方式/目录）
        if super::helpers::check_software_detected(vi) {
            if let Some(ref mut c) = child {
                let _ = c.kill();
                let _ = c.wait();
            }
            println!();
            return Ok((true, None));
        }

        // 2. 检查进程是否已退出（用于捕获权限不足等快速失败）
        if let Some(ref mut c) = child {
            if let Ok(Some(status)) = c.try_wait() {
                if status.success() {
                    // 进程退出且返回成功 → 快速轮询确认
                    child = None;
                } else {
                    let code = status.code().unwrap_or(-1);
                    if code == 1223 || code == 740 {
                        let _ = c.kill();
                        let _ = c.wait();
                        println!();
                        println!("  需要管理员权限，尝试提权...");
                        return try_elevate_and_detect(&actual_installer, &vi.install_args, vi);
                    }
                    // 非零退出 → 可能还是装了，继续检测直到超时
                    child = None;
                }
            }
        }

        // 3. 超时检查
        if start.elapsed() > timeout {
            if let Some(ref mut c) = child {
                let _ = c.kill();
                let _ = c.wait();
            }
            // 最后一次检测
            if super::helpers::check_software_detected(vi) {
                println!();
                return Ok((true, None));
            }
            println!();
            eprintln!("  安装超时（{} 秒），未检测到安装", INSTALLER_TIMEOUT_SECS);
            return Ok((false, None));
        }

        // 4. 心跳
        if !printed_dot {
            print!("  ");
            printed_dot = true;
        }
        print!(".");
        let _ = std::io::Write::flush(&mut std::io::stdout());

        std::thread::sleep(Duration::from_millis(500));
    }
}

/// 回退方案：没有任何检测配置时，只能等待进程退出。
fn fallback_wait_process(
    child: &mut Option<std::process::Child>,
    installer_path: &Path,
    vi: &crate::software::VersionInfo,
) -> anyhow::Result<(bool, Option<PathBuf>)> {
    let mut child = match child.take() {
        Some(c) => c,
        None => return Ok((false, None)),
    };
    match child.wait() {
        Ok(s) if s.success() => Ok((true, None)),
        Ok(s) => {
            let code = s.code().unwrap_or(-1);
            if code == 1223 || code == 740 {
                println!("  需要管理员权限，尝试提权...");
                return try_elevate_and_detect(installer_path, &vi.install_args, vi);
            }
            eprintln!("  安装程序返回错误码 {}", code);
            Ok((false, None))
        }
        Err(e) => {
            eprintln!("  等待安装进程失败: {}", e);
            Ok((false, None))
        }
    }
}

/// 提权运行安装器后，继续轮询检测。
fn try_elevate_and_detect(
    installer_path: &Path,
    args: &[String],
    vi: &crate::software::VersionInfo,
) -> anyhow::Result<(bool, Option<PathBuf>)> {
    let mut ps_args = format!(
        "Start-Process -FilePath '{}'",
        installer_path.display()
    );
    if !args.is_empty() {
        let arg_str = args.iter()
            .map(|a| format!("'{}'", a.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(", ");
        ps_args.push_str(&format!(" -ArgumentList {}", arg_str));
    }
    ps_args.push_str(" -Verb RunAs");

    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_args])
        .spawn();

    println!("  等待提权安装完成...");

    let timeout = Duration::from_secs(INSTALLER_TIMEOUT_SECS);
    let start = Instant::now();
    let mut printed_dot = false;

    loop {
        if super::helpers::check_software_detected(vi) {
            println!();
            return Ok((true, None));
        }
        if start.elapsed() > timeout {
            println!();
            eprintln!("  提权安装超时（{} 秒），未检测到安装", INSTALLER_TIMEOUT_SECS);
            return Ok((false, None));
        }
        if !printed_dot {
            print!("  ");
            printed_dot = true;
        }
        print!(".");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// 将压缩包解压到指定目录（第三方软件安装版用）。
/// 不处理单根目录展平，保持原始目录结构。
fn extract_archive_to(archive_path: &Path, target_dir: &Path) -> anyhow::Result<()> {
    let ext = archive_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext.to_lowercase().as_str() {
        "zip" => {
            let status = Command::new("powershell")
                .args([
                    "-NoProfile", "-Command",
                    &format!(
                        "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                        archive_path.display(), target_dir.display()
                    ),
                ])
                .status()?;
            if !status.success() {
                bail!("解压 zip 失败");
            }
        }
        _ => {
            let candidates = [
                paths::builds_dir().join("7zr").join("7zr.exe"),
                paths::builds_dir().join("7zr.exe"),
                PathBuf::from("C:\\Program Files\\7-Zip\\7z.exe"),
                PathBuf::from("C:\\Program Files (x86)\\7-Zip\\7z.exe"),
            ];
            let seven_z = candidates.iter().find(|p| p.exists());
            let status = if let Some(exe) = seven_z {
                Command::new(exe)
                    .args(["x", &archive_path.to_string_lossy(), &format!("-o{}", target_dir.display()), "-y"])
                    .status()?
            } else {
                bail!("不支持的压缩格式 '{}'（未找到解压工具）。\n  提示：请安装 7-Zip 或将 7zr.exe 放入 {}",
                    ext, paths::builds_dir().display())
            };
            if !status.success() {
                bail!("解压失败");
            }
        }
    }
    Ok(())
}

/// 安装便携版（第三方软件）。
pub(crate) fn install_portable(name: &str, version: &str, archive_path: &Path) -> anyhow::Result<PathBuf> {
    let dir_name = format!("{}-{}", name, version);
    let target = paths::apps_dir().join(&dir_name);
    if target.exists() {
        println!("  便携版目录已存在，覆盖安装...");
        fs::remove_dir_all(&target)?;
    }

    let staging = target.with_extension("staging");
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;

    let ext = archive_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    println!("  解压中 ...");

    match ext.to_lowercase().as_str() {
        "zip" => {
            let status = Command::new("powershell")
                .args([
                    "-NoProfile", "-Command",
                    &format!(
                        "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                        archive_path.display(), staging.display()
                    ),
                ])
                .status()?;
            if !status.success() {
                bail!("解压 zip 失败");
            }
        }
        _ => {
            let candidates = [
                paths::builds_dir().join("7zr").join("7zr.exe"),
                paths::builds_dir().join("7zr.exe"),
                PathBuf::from("C:\\Program Files\\7-Zip\\7z.exe"),
                PathBuf::from("C:\\Program Files (x86)\\7-Zip\\7z.exe"),
            ];
            let seven_z = candidates.iter().find(|p| p.exists());
            let status = if let Some(exe) = seven_z {
                Command::new(exe)
                    .args(["x", &archive_path.to_string_lossy(), &format!("-o{}", staging.display()), "-y"])
                    .status()?
            } else {
                bail!("不支持的压缩格式 '{}'（未找到解压工具）。\n  提示：请安装 7-Zip 或将 7zr.exe 放入 {}",
                    ext, paths::builds_dir().display())
            };
            if !status.success() {
                bail!("解压失败");
            }
        }
    }

    let entries: Vec<_> = fs::read_dir(&staging)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            !name.starts_with('.') && !name.starts_with("__MACOSX")
        })
        .collect();

    if entries.is_empty() {
        fs::remove_dir(&staging)?;
        bail!("压缩包为空或仅包含系统文件");
    }

    if entries.len() == 1 && entries[0].file_type().map(|t| t.is_dir()).unwrap_or(false) {
        let single_dir = entries[0].path();
        fs::rename(&single_dir, &target)?;
    } else {
        fs::create_dir(&target)?;
        for entry in &entries {
            let src = entry.path();
            let dest = target.join(entry.file_name());
            fs::rename(&src, &dest)?;
        }
    }

    let _ = fs::remove_dir(&staging);
    println!("  已解压到 {}", target.display());
    Ok(target)
}

/// 将压缩包解压到指定目录（自研工具用）。
/// 如果压缩包内只有一个根目录，则提取该目录的内容；否则直接解压到目标目录。
pub fn extract_zip_to(archive_path: &Path, target_dir: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(target_dir)?;

    let ext = archive_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let staging = target_dir.with_extension("staging");
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;

    match ext.as_str() {
        "zip" => {
            let status = Command::new("powershell")
                .args([
                    "-NoProfile", "-Command",
                    &format!(
                        "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                        archive_path.display(), staging.display()
                    ),
                ])
                .status()?;
            if !status.success() {
                bail!("解压 zip 失败");
            }
        }
        _ => {
            let candidates = [
                paths::builds_dir().join("7zr").join("7zr.exe"),
                paths::builds_dir().join("7zr.exe"),
                PathBuf::from("C:\\Program Files\\7-Zip\\7z.exe"),
                PathBuf::from("C:\\Program Files (x86)\\7-Zip\\7z.exe"),
            ];
            let seven_z = candidates.iter().find(|p| p.exists());
            let status = if let Some(exe) = seven_z {
                Command::new(exe)
                    .args(["x", &archive_path.to_string_lossy(), &format!("-o{}", staging.display()), "-y"])
                    .status()?
            } else {
                bail!("不支持的压缩格式 '{}'（未找到解压工具）", ext);
            };
            if !status.success() {
                bail!("解压失败");
            }
        }
    }

    // 检查 staging 是否只有一个根目录
    let entries: Vec<_> = fs::read_dir(&staging)?
        .filter_map(|e| e.ok())
        .collect();

    if entries.len() == 1 && entries[0].file_type().map(|t| t.is_dir()).unwrap_or(false) {
        let inner = entries[0].path();
        for entry in fs::read_dir(&inner)? {
            let entry = entry?;
            let target = target_dir.join(entry.file_name());
            let _ = fs::remove_dir_all(&target);
            fs::rename(entry.path(), &target)?;
        }
    } else {
        for entry in entries {
            let target = target_dir.join(entry.file_name());
            let _ = fs::remove_dir_all(&target);
            fs::rename(entry.path(), &target)?;
        }
    }

    let _ = fs::remove_dir_all(&staging);
    Ok(())
}
