use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Detect if an executable is an NSIS installer by scanning for "Nullsoft" bytes.
pub(crate) fn is_nsis(path: &Path) -> bool {
    let mut f = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut buf = [0u8; 65536];
    while let Ok(n) = f.read(&mut buf) {
        if n == 0 {
            break;
        }
        if buf[..n].windows(8).any(|w| w == b"Nullsoft") {
            return true;
        }
    }
    false
}

/// Detect if an executable is an Inno Setup installer.
pub(crate) fn is_inno(path: &Path) -> bool {
    let mut f = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut buf = [0u8; 65536];
    while let Ok(n) = f.read(&mut buf) {
        if n == 0 {
            break;
        }
        if buf[..n].windows(9).any(|w| w == b"Inno Setup") {
            return true;
        }
    }
    false
}

/// Detect installer type. Returns "nsis", "inno", or empty string if unknown.
pub fn detect_installer_type(path: &Path) -> &'static str {
    if is_nsis(path) {
        "nsis"
    } else if is_inno(path) {
        "inno"
    } else {
        ""
    }
}

/// 检测单个 exe 文件的安装器类型：msi / nsis / inno / ""。
/// path 为 None 或文件不存在时返回 ""。
pub(crate) fn detect_file_type(path: Option<&str>) -> &'static str {
    let path = match path {
        Some(p) => Path::new(p),
        None => return "",
    };

    // 先按文件名检测（不依赖文件存在性），msiexec 在 PATH 上不是本地文件
    let fname = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    if fname == "msiexec.exe" || fname == "msiexec" {
        return "msi";
    }

    if !path.is_file() {
        return "";
    }

    // MSI: .msi 后缀
    if path.extension().map_or(false, |e| e.eq_ignore_ascii_case("msi")) {
        return "msi";
    }

    let fname = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Inno Setup 标准命名：unins000.exe
    if fname.starts_with("unins") && fname.ends_with(".exe") {
        return "inno";
    }

    // 二进制扫描
    if is_inno(path) {
        return "inno";
    }
    if is_nsis(path) {
        return "nsis";
    }
    ""
}

/// 检查两个路径是否指向同一文件（通过 canonicalize）。
pub(crate) fn same_file(a: &Path, b: &Path) -> bool {
    let ca = a.canonicalize();
    let cb = b.canonicalize();
    match (ca, cb) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// 在安装目录中查找真正的卸载程序（绕过 stub）。
///
/// Inno Setup 通常会在安装目录生成 `unins000.exe` / `unins001.exe` 等，
/// 而 UninstallString 指向的往往是转发 stub。
/// 此函数在 InstallLocation 中查找真实卸载器并检测其类型。
pub(crate) fn resolve_real_uninstaller(uninstall_args: &[String], install_path: Option<&str>) -> (Vec<String>, &'static str) {
    // 先检测 UninstallString 指向的 exe
    let stub_type = detect_file_type(uninstall_args.first().map(|s| s.as_str()));
    if stub_type != "" {
        // stub 本身就有明确的安装器类型，直接用它
        return (uninstall_args.to_vec(), stub_type);
    }

    let install_dir = match install_path {
        Some(p) => Path::new(p).to_path_buf(),
        None => {
            // 注册表中没有 InstallLocation 时，从 UninstallString 的目录推导
            if let Some(first) = uninstall_args.first() {
                let p = Path::new(first);
                if let Some(parent) = p.parent() {
                    if parent.is_dir() {
                        parent.to_path_buf()
                    } else {
                        return (uninstall_args.to_vec(), "");
                    }
                } else {
                    return (uninstall_args.to_vec(), "");
                }
            } else {
                return (uninstall_args.to_vec(), "");
            }
        }
    };

    if !install_dir.is_dir() {
        return (uninstall_args.to_vec(), "");
    }

    let mut fallback_candidate: Option<PathBuf> = None;

    // 在安装目录中查找已知的卸载程序名
    let candidates = ["unins000.exe", "unins001.exe", "Uninstall.exe", "uninstall.exe"];
    for name in &candidates {
        let candidate = install_dir.join(name);
        if !candidate.is_file() {
            continue;
        }
        // 跳过和 UninstallString 指向相同文件的
        if let Some(stub_path) = uninstall_args.first() {
            if same_file(&candidate, Path::new(stub_path)) {
                continue;
            }
        }
        let ftype = detect_file_type(Some(candidate.to_str().unwrap_or("")));
        if ftype != "" {
            let mut real_args = vec![candidate.to_string_lossy().to_string()];
            if uninstall_args.len() > 1 {
                real_args.extend_from_slice(&uninstall_args[1..]);
            }
            return (real_args, ftype);
        }
        if fallback_candidate.is_none() {
            fallback_candidate = Some(candidate);
        }
    }

    // 检查 unins???.exe 通配
    if let Ok(entries) = fs::read_dir(&install_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let fname = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if fname.starts_with("unins") && fname.ends_with(".exe") {
                if let Some(stub_path) = uninstall_args.first() {
                    if same_file(&path, Path::new(stub_path)) {
                        continue;
                    }
                }
                let ftype = detect_file_type(Some(path.to_str().unwrap_or("")));
                if ftype != "" {
                    let mut real_args = vec![path.to_string_lossy().to_string()];
                    if uninstall_args.len() > 1 {
                        real_args.extend_from_slice(&uninstall_args[1..]);
                    }
                    return (real_args, ftype);
                }
                if fallback_candidate.is_none() {
                    fallback_candidate = Some(path);
                }
            }
        }
    }

    // 类型未知但文件存在，作为回退使用
    if let Some(fb) = fallback_candidate {
        let mut real_args = vec![fb.to_string_lossy().to_string()];
        if uninstall_args.len() > 1 {
            real_args.extend_from_slice(&uninstall_args[1..]);
        }
        return (real_args, "");
    }

    // 回退：用原始 UninstallString
    (uninstall_args.to_vec(), "")
}
