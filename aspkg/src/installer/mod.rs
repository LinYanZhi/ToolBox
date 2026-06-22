use std::path::Path;
use std::fs;
use std::io::{Read, Write};
use anyhow::Context;

/// 安装便携版：解压到 apps/{name}-{version}/ 目录
pub fn install_portable(name: &str, version: &str, dl_path: &Path) -> anyhow::Result<String> {
    let target_dir = crate::paths::apps_dir().join(format!("{}-{}", name, version));

    // 如果目标目录已存在（更新/重装），先清理干净
    if target_dir.is_dir() {
        std::fs::remove_dir_all(&target_dir)
            .with_context(|| format!("无法清理旧版目录 {}，请确认程序是否正在运行", target_dir.display()))?;
    }
    std::fs::create_dir_all(&target_dir)?;

    // 解压/复制
    let archive_type = detect_archive_type(dl_path);
    match archive_type {
        "zip" => extract_zip(dl_path, &target_dir)?,
        "7z" => extract_7z(dl_path, &target_dir)?,
        _ => {
            // 单文件便携版（exe/msi 或其他）→ 直接复制
            let dest = target_dir.join(dl_path.file_name().unwrap());
            std::fs::copy(dl_path, &dest)?;
        }
    }

    // 查找主 exe 并创建快捷桩
    if let Some(entry_exe) = find_entry_exe(name, &target_dir) {
        create_shim(name, &entry_exe);
    }

    println!("  便携版已安装到: {}", target_dir.display());
    Ok(target_dir.to_string_lossy().to_string())
}

/// 通过文件头魔数检测下载文件的实际类型（不受扩展名限制）。
pub(crate) fn detect_archive_type(path: &Path) -> &'static str {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // 扩展名可信时直接返回
    match ext.as_str() {
        "zip" => return "zip",
        "7z" => return "7z",
        "tar" => return "tar",
        "exe" | "msi" => return "single",
        _ => {}
    }

    // 扩展名不可信/缺失 → 读魔数
    let mut buf = [0u8; 8];
    if let Ok(mut f) = fs::File::open(path) {
        if f.read_exact(&mut buf).is_ok() {
            // ZIP 魔数: PK\x03\x04
            if buf[..4] == [0x50, 0x4B, 0x03, 0x04] {
                return "zip";
            }
            // 7z 魔数: 7z\xBC\xAF\x27\x1C
            if buf[..6] == [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C] {
                return "7z";
            }
            // PE (exe) 魔数: MZ
            if buf[..2] == [0x4D, 0x5A] {
                return "single";
            }
        }
    }

    // 无法识别 → 按单文件处理
    "single"
}

/// 安装安装版：运行安装器
///
/// `detect` 是软件源的注册表检测配置，用于安装后验证是否真的安装成功了。
/// 如果静默安装失败（用户取消/不支持静默），会询问用户是否直接显示安装界面。
pub fn install_installer(
    name: &str,
    _version: &str,
    dl_path: &Path,
    detect: Option<&crate::software::DetectConfig>,
) -> anyhow::Result<String> {
    let ext = dl_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // 启动安装程序，保存子进程句柄用于后续监控
    let mut child = match ext.as_str() {
        "msi" => {
            println!("  正在启动 MSI 安装程序（请在弹出的窗口中完成安装）...");
            std::process::Command::new("msiexec")
                .args(["/i", &dl_path.to_string_lossy()])
                .spawn()
                .context("启动 msiexec 失败")?
        }
        "exe" => {
            println!("  正在启动安装程序（请在弹出的窗口中完成安装）...");
            std::process::Command::new(dl_path)
                .spawn()
                .context("启动安装程序失败")?
        }
        _ => {
            // 未知扩展名 → 用魔数检测实际类型
            let real = detect_archive_type(dl_path);
            match real {
                "single" => {
                    let exe_path = dl_path.with_extension("exe");
                    let _ = std::fs::rename(dl_path, &exe_path);
                    return install_installer(name, _version, &exe_path, detect);
                }
                "zip" | "7z" => {
                    return install_compressed_installer(name, _version, dl_path, real, detect);
                }
                "tar" => {
                    return install_tar_installer(name, _version, dl_path, detect);
                }
                _ => anyhow::bail!("不支持的安装包类型: .{}", ext),
            }
        }
    };

    // ── 轮询检测安装是否成功 ────────────────────────
    print!("  等待安装完成（安装程序关闭后将自动继续）");
    let _ = std::io::stdout().flush();
    for i in 0..150 {
        // 每 2 秒一次，最多等 5 分钟

        // 检测安装程序进程是否已退出
        match child.try_wait() {
            Ok(Some(status)) => {
                // 进程已退出
                println!();
                println!("  安装程序已退出 (退出码: {:?})", status.code());
                // 再查一次注册表确认是否安装成功
                let found = check_registry(name, detect, dl_path);
                if found.is_some() {
                    return Ok(found.unwrap());
                }
                println!("  {} 注册表中未检测到 {} 的安装记录", color::yellow("提示"), name);
                println!("    请手动确认安装是否已完成");
                return Ok("(用户确认)".to_string());
            }
            Ok(None) => {} // 进程仍在运行，继续等待
            Err(_) => {
                // 无法检测进程状态 → 继续等待（不回显错误）
            }
        }

        let found = check_registry(name, detect, dl_path);
        if let Some(path) = found {
            println!();
            return Ok(path);
        }

        if i > 0 && i % 15 == 0 {
            let secs = (i + 1) * 2;
            print!(" 已等待 {} 秒", secs);
        } else {
            print!(".");
        }
        let _ = std::io::stdout().flush();
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    anyhow::bail!(
        "等待注册超时（已等待 5 分钟），注册表中未检测到 {}，可能并未真正安装成功",
        name
    );
}

/// 在注册表中检测指定软件是否已安装。
fn check_registry(
    name: &str,
    detect: Option<&crate::software::DetectConfig>,
    dl_path: &Path,
) -> Option<String> {
    if let Some(detect_cfg) = detect {
        crate::software::detect_from_registry_raw(detect_cfg)
            .map(|info| {
                let install_path = info.install_path.unwrap_or_else(|| {
                    format!("{} (系统程序)", dl_path.to_string_lossy())
                });
                if !info.version.is_empty() {
                    println!("  检测到已安装版本: {}", info.version);
                }
                install_path
            })
    } else {
        sys::registry::detect_installed_by(name, None)
            .and_then(|m| m.get("InstallLocation").cloned())
            .map(|p| {
                if !p.is_empty() { p } else {
                    format!("{} (系统程序)", dl_path.to_string_lossy())
                }
            })
    }
}

/// 解压 tar 包并执行其中的 exe 安装程序
///
/// VMware 等厂商以 .exe.tar 格式分发安装程序。
/// 使用 Windows 内置 tar.exe 解压，找到 .exe 后递归执行安装。
fn install_tar_installer(
    name: &str,
    _version: &str,
    dl_path: &Path,
    detect: Option<&crate::software::DetectConfig>,
) -> anyhow::Result<String> {
    // 创建临时解压目录
    let extract_dir = dl_path.parent().unwrap_or(Path::new(".")).join(format!("{}_extracted", name));
    let _ = std::fs::remove_dir_all(&extract_dir);
    std::fs::create_dir_all(&extract_dir)?;

    println!("  解压 tar 包...");
    let status = std::process::Command::new("tar")
        .args(["-xf", &dl_path.to_string_lossy(), "-C", &extract_dir.to_string_lossy()])
        .status()
        .map_err(|e| anyhow::anyhow!("tar.exe 解压失败: {} (Windows 10 1803+ 自带 tar.exe)", e))?;

    if !status.success() {
        let _ = std::fs::remove_dir_all(&extract_dir);
        anyhow::bail!("tar 解压失败 (退出码: {:?})", status.code());
    }

    // 在解压目录中查找 exe
    let exe = find_exe_recursive(&extract_dir).ok_or_else(|| {
        let _ = std::fs::remove_dir_all(&extract_dir);
        anyhow::anyhow!("tar 包内未找到 exe 安装程序")
    })?;

    println!("  找到安装程序: {}", exe.file_name().unwrap_or_default().to_string_lossy());

    // 把 exe 复制到下载目录（zip 旁边），避免 temp 目录被清理后文件丢失
    let target_exe = dl_path.parent().unwrap_or(Path::new(".")).join(
        exe.file_name().unwrap_or_default()
    );
    std::fs::copy(&exe, &target_exe)?;

    // 清理解压目录
    let _ = std::fs::remove_dir_all(&extract_dir);

    // 递归调用 install_installer（用稳定路径）
    install_installer(name, _version, &target_exe, detect)
}

/// 解压 zip/7z 压缩包并执行其中的 exe 安装程序
///
/// VMware 等厂商以 .zip 格式分发安装程序。
/// 支持 zip 和 7z 两种格式，解压后找到 exe 递归执行安装。
fn install_compressed_installer(
    name: &str,
    _version: &str,
    dl_path: &Path,
    archive_type: &str,
    detect: Option<&crate::software::DetectConfig>,
) -> anyhow::Result<String> {
    // 创建临时解压目录
    let extract_dir = dl_path.parent().unwrap_or(Path::new(".")).join(format!("{}_extracted", name));
    let _ = std::fs::remove_dir_all(&extract_dir);
    std::fs::create_dir_all(&extract_dir)?;

    if archive_type == "zip" {
        extract_zip(dl_path, &extract_dir)?;
    } else if archive_type == "7z" {
        // 使用 7-Zip 解压
        println!("  解压 7z 包...");
        let status = std::process::Command::new("7z")
            .args(["x", &dl_path.to_string_lossy(), &format!("-o{}", extract_dir.to_string_lossy()), "-y"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| anyhow::anyhow!("7z 解压失败: {}", e))?;

        if !status.success() {
            let _ = std::fs::remove_dir_all(&extract_dir);
            anyhow::bail!("7z 解压失败 (退出码: {:?})", status.code());
        }
    }

    // 在解压目录中查找 exe
    let exe = find_exe_recursive(&extract_dir).ok_or_else(|| {
        let _ = std::fs::remove_dir_all(&extract_dir);
        anyhow::anyhow!("压缩包内未找到 exe 安装程序")
    })?;

    println!("  找到安装程序: {}", exe.file_name().unwrap_or_default().to_string_lossy());

    // 递归调用 install_installer
    let result = install_installer(name, _version, &exe, detect);

    // 清理解压目录
    let _ = std::fs::remove_dir_all(&extract_dir);

    result
}

/// 递归查找目录下的安装程序（只匹配已知的安装文件名）。
///
/// 匹配规则（不区分大小写）：
///   - exe: setup.exe, install.exe, vcredist_*.exe, vc_redist*.exe, dotnet*.exe, *-amd64.exe
///   - msi: 所有 .msi 文件
/// 避免误匹配到 readme.exe 等无关文件。
fn find_exe_recursive(dir: &Path) -> Option<std::path::PathBuf> {
    if dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        let ext_lower = ext.to_str()?.to_lowercase();
                        // .msi 直接匹配
                        if ext_lower == "msi" {
                            return Some(path);
                        }
                        // .exe 按文件名过滤
                        if ext_lower == "exe" {
                            if let Some(stem) = path.file_stem()?.to_str() {
                                let stem_lower = stem.to_lowercase();
                                // 已知安装程序文件名模式
                                if stem_lower == "setup"
                                    || stem_lower == "install"
                                    || stem_lower.starts_with("vcredist")
                                    || stem_lower.starts_with("vc_redist")
                                    || stem_lower.starts_with("dotnet")
                                    || stem_lower.ends_with("-amd64")
                                    || stem_lower.ends_with("-x64")
                                    || stem_lower.ends_with("-x86")
                                    || stem_lower.starts_with("unins")
                                    || stem_lower == "uninstall"
                                {
                                    return Some(path);
                                }
                            }
                        }
                    }
                } else if path.is_dir() {
                    if let Some(found) = find_exe_recursive(&path) {
                        return Some(found);
                    }
                }
            }
        }
    }
    None
}

/// 使用 PowerShell 解压 zip（静默尝试两种方法，只对最终失败报错）
fn extract_zip(zip_path: &Path, target: &Path) -> anyhow::Result<()> {
    // 不使用 canonicalize，避免 PowerShell 不兼容 \\?\ 前缀路径
    let zip_str = zip_path.to_string_lossy();
    let target_str = target.to_string_lossy();

    println!("  解压 {} -> {}", color::gray(&zip_str), color::gray(&target_str));

    // 方法一：Expand-Archive（支持 -Force 覆盖，兼容性好）
    let ok1 = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                zip_str, target_str
            ),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if ok1 && is_nonempty_dir(target) {
        return Ok(());
    }

    // 方法二：.NET ZipFile（2 参数版本，无 overwrite 标志）
    // 注意：.NET Framework 4.x 的 3 参数重载不接受 bool（会与 Encoding 混淆），
    // 所以用 2 参数版，并提前清理目标目录
    let _ = fs::remove_dir_all(target);
    let _ = fs::create_dir_all(target);
    let ok2 = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Add-Type -AssemblyName System.IO.Compression.FileSystem; \
                 [System.IO.Compression.ZipFile]::ExtractToDirectory('{}', '{}')",
                zip_str, target_str
            ),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if ok2 && is_nonempty_dir(target) {
        return Ok(());
    }

    anyhow::bail!("解压失败：无法解压 {}，请确认文件是否损坏", zip_str)
}

/// 检查目录是否存在且非空。
fn is_nonempty_dir(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    match dir.read_dir() {
        Ok(mut iter) => iter.next().is_some(),
        Err(_) => false,
    }
}

/// 解压 7z 文件（尝试多个工具）
fn extract_7z(path: &Path, target: &Path) -> anyhow::Result<()> {
    // 尝试系统 7z
    if let Ok(status) = std::process::Command::new("7z")
        .args(["x", &path.to_string_lossy(), &format!("-o{}", target.to_string_lossy()), "-y"])
        .status()
    {
        if status.success() {
            return Ok(());
        }
    }

    // 尝试 7-Zip 默认安装路径
    let seven_zip = Path::new(r"C:\Program Files\7-Zip\7z.exe");
    if seven_zip.is_file() {
        if let Ok(status) = std::process::Command::new(seven_zip)
            .args(["x", &path.to_string_lossy(), &format!("-o{}", target.to_string_lossy()), "-y"])
            .status()
        {
            if status.success() {
                return Ok(());
            }
        }
    }

    anyhow::bail!("无法解压 .7z 文件，请安装 7-Zip 或将 7z.exe 加入 PATH")
}

/// 在安装目录中查找与软件名匹配的主 exe 文件（递归搜索）。
fn find_entry_exe(name: &str, dir: &Path) -> Option<String> {
    let mut exe_files: Vec<String> = Vec::new();
    collect_exe_files(dir, &mut exe_files);

    if exe_files.is_empty() {
        return None;
    }
    // 只有一个 exe → 直接使用
    if exe_files.len() == 1 {
        return Some(exe_files.remove(0));
    }
    // 多个 exe → 优先匹配软件名（不含扩展名）
    let name_lower = name.to_lowercase();
    for exe in &exe_files {
        let stem = Path::new(exe).file_stem()?.to_str()?.to_lowercase();
        if stem == name_lower {
            return Some(exe.clone());
        }
    }
    // 还是没匹配到 → 跳过，不创建 shim
    None
}

/// 递归收集目录下所有 .exe 文件路径。
fn collect_exe_files(dir: &Path, result: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_exe_files(&path, result);
            } else if path.extension().and_then(|e| e.to_str()) == Some("exe") {
                result.push(path.to_string_lossy().to_string());
            }
        }
    }
}

/// 在 %LOCALAPPDATA%/aminos/bin/ 下为便携版软件创建 .bat 快捷桩。
///
/// 桩文件内容：
/// ```bat
/// @echo off
/// "C:\full\path\to\real.exe" %*
/// ```
fn create_shim(name: &str, exe_path: &str) {
    let bin_dir = crate::paths::bin_dir();
    if let Err(e) = fs::create_dir_all(&bin_dir) {
        eprintln!("  {} 无法创建快捷桩目录: {}", color::yellow("警告"), e);
        return;
    }

    let shim_path = bin_dir.join(format!("{}.bat", name));
    let content = format!(
        "@echo off\r\n\"{}\" %*\r\n",
        exe_path
    );

    if let Err(e) = fs::write(&shim_path, content) {
        eprintln!("  {} 无法创建快捷桩: {}", color::yellow("警告"), e);
        return;
    }
}

/// 检查 PATH 中是否包含 bin 目录，若不包含则打印提示。
pub fn check_bin_path_warning() {
    let bin_dir = crate::paths::bin_dir();
    let bin_str = bin_dir.to_string_lossy().to_lowercase();

    if let Some(path_var) = std::env::var_os("PATH") {
        if let Some(path_str) = path_var.to_str() {
            let in_path = path_str.split(';').any(|p| {
                let p = p.trim_matches('"').to_lowercase();
                p == bin_str
            });
            if in_path {
                return;
            }
        }
    }

    eprintln!("  {} bin 目录不在 PATH 中，部分命令行工具可能无法直接使用", color::yellow("⚠ 提示"));
    eprintln!("    请手动将以下路径添加到系统/用户 PATH 环境变量：");
    eprintln!("    {}", color::cyan(&bin_dir.display().to_string()));
}
