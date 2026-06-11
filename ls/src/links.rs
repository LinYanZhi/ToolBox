use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::process::Command;

/// 链接类型
#[derive(Debug, Clone, PartialEq)]
pub enum LinkType {
    /// 普通目录
    Dir,
    /// 普通文件
    File,
    /// 符号链接（目录或文件）
    Symlink,
    /// Windows 目录连接点
    Junction,
    /// .lnk 快捷方式
    Shortcut,
    /// 未知
    Unknown,
}

/// 链接信息
#[derive(Debug, Clone)]
pub struct LinkInfo {
    pub link_type: LinkType,
    pub target: Option<String>,
}

/// 检测路径是否为 Junction
pub fn is_junction(path: &Path) -> bool {
    let path_wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // SAFETY: 标准 Windows API 调用
    let attrs = unsafe { kernel32::GetFileAttributesW(path_wide.as_ptr()) };
    if attrs == u32::MAX {
        return false;
    }
    (attrs & 0x400) != 0
}

/// 获取 Junction 的目标路径
pub fn get_junction_target(path: &Path) -> Option<String> {
    let output = Command::new("fsutil")
        .args(["reparsepoint", "query", &path.to_string_lossy()])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(target) = line.strip_prefix("打印名称:").or(line.strip_prefix("Print Name:")) {
            return Some(normalize_path(target.trim()));
        }
        if let Some(target) = line.strip_prefix("替换名称:").or(line.strip_prefix("Substitute Name:")) {
            return Some(normalize_path(target.trim()));
        }
    }
    None
}

/// 从 UTF-16LE 字节创建 String（替代不稳定的 from_utf16le）
fn from_utf16le(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 2 || bytes.len() % 2 != 0 {
        return None;
    }
    let mut chars = Vec::with_capacity(bytes.len() / 2);
    for i in (0..bytes.len()).step_by(2) {
        let code = u16::from_le_bytes([bytes[i], bytes[i + 1]]);
        if code == 0 {
            break;
        }
        chars.push(code);
    }
    String::from_utf16(&chars).ok()
}

/// 获取 .lnk 快捷方式的目标路径
pub fn get_lnk_target(path: &Path) -> Option<String> {
    let data = std::fs::read(path).ok()?;
    if data.len() < 76 {
        return None;
    }

    // 验证 Shell Link 头部
    let header_size = u32::from_le_bytes(data[0..4].try_into().ok()?);
    if header_size != 0x4C {
        return None;
    }

    let clsid = &data[4..20];
    let expected_clsid = &[
        0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
    ];
    if clsid != expected_clsid {
        return None;
    }

    let link_flags = u32::from_le_bytes(data[20..24].try_into().ok()?);
    let has_link_info = (link_flags & 0x02) != 0;
    let has_relative_path = (link_flags & 0x08) != 0;
    let has_working_dir = (link_flags & 0x10) != 0;
    let is_unicode = (link_flags & 0x80) != 0;
    let force_no_link_info = (link_flags & 0x100) != 0;

    let mut offset: usize = 76;

    // 跳过 LinkTargetIDList
    if (link_flags & 0x01) != 0 {
        if offset + 2 > data.len() {
            return None;
        }
        let id_list_size = u16::from_le_bytes(data[offset..offset + 2].try_into().ok()?) as usize;
        offset += 2 + id_list_size;
    }

    // 解析 LinkInfo
    let mut link_info_target: Option<String> = None;

    if has_link_info && !force_no_link_info {
        if offset + 4 <= data.len() {
            let link_info_start = offset;
            let link_info_size = u32::from_le_bytes(data[offset..offset + 4].try_into().ok()?);
            let link_info_end = link_info_start + link_info_size as usize;

            if link_info_end <= data.len() && link_info_size >= 0x1C {
                let header_size = u32::from_le_bytes(data[offset + 4..offset + 8].try_into().ok()?);
                let _link_info_flags = u32::from_le_bytes(data[offset + 8..offset + 12].try_into().ok()?);

                let _volume_id_offset = u32::from_le_bytes(data[offset + 12..offset + 16].try_into().ok()?);
                let local_base_path_offset = u32::from_le_bytes(data[offset + 16..offset + 20].try_into().ok()?);
                let common_path_suffix_offset =
                    u32::from_le_bytes(data[offset + 24..offset + 28].try_into().ok()?);

                let local_base_path_offset_unicode: u32;
                let common_path_suffix_offset_unicode: u32;

                if header_size >= 0x24 {
                    local_base_path_offset_unicode =
                        u32::from_le_bytes(data[offset + 28..offset + 32].try_into().ok()?);
                    common_path_suffix_offset_unicode =
                        u32::from_le_bytes(data[offset + 32..offset + 36].try_into().ok()?);
                } else {
                    local_base_path_offset_unicode = 0;
                    common_path_suffix_offset_unicode = 0;
                }

                let local_base_path = if local_base_path_offset_unicode > 0 {
                    from_utf16le(&data[link_info_start + local_base_path_offset_unicode as usize..])
                } else if local_base_path_offset > 0 && local_base_path_offset < link_info_size {
                    read_ansi_string(&data, link_info_start + local_base_path_offset as usize)
                } else {
                    None
                };

                let common_path_suffix = if common_path_suffix_offset_unicode > 0 {
                    from_utf16le(
                        &data[link_info_start + common_path_suffix_offset_unicode as usize..],
                    )
                } else if common_path_suffix_offset > 0 && common_path_suffix_offset < link_info_size {
                    read_ansi_string(&data, link_info_start + common_path_suffix_offset as usize)
                } else {
                    None
                };

                link_info_target = match (local_base_path, common_path_suffix) {
                    (Some(base), Some(suffix)) => {
                        if base.ends_with('\\') {
                            Some(format!("{}{}", base, suffix))
                        } else {
                            Some(format!("{}\\{}", base, suffix))
                        }
                    }
                    (Some(base), None) => Some(base),
                    (None, Some(suffix)) => Some(suffix),
                    (None, None) => None,
                };
            }

            offset = link_info_end;
        }
    }

    if let Some(target) = link_info_target {
        return Some(target);
    }

    // 读取名称字符串
    if (link_flags & 0x04) != 0 {
        let (_, new_offset) = read_string_data(&data, offset, is_unicode)?;
        offset = new_offset;
    }

    // 读取相对路径或工作目录
    if has_relative_path {
        let (rel_path, new_offset) = read_string_data(&data, offset, is_unicode)?;
        offset = new_offset;
        if !rel_path.is_empty() {
            return Some(rel_path);
        }
    }

    if has_working_dir {
        let (work_dir, _) = read_string_data(&data, offset, is_unicode)?;
        if !work_dir.is_empty() {
            return Some(work_dir);
        }
    }

    None
}

fn read_ansi_string(data: &[u8], offset: usize) -> Option<String> {
    let end = data[offset..].iter().position(|&b| b == 0)?;
    let bytes = &data[offset..offset + end];
    Some(String::from_utf8_lossy(bytes).to_string())
}

fn read_string_data(data: &[u8], offset: usize, is_unicode: bool) -> Option<(String, usize)> {
    if offset + 2 > data.len() {
        return None;
    }
    if is_unicode {
        let char_count = u16::from_le_bytes(data[offset..offset + 2].try_into().ok()?) as usize;
        let byte_len = char_count * 2;
        if offset + 2 + byte_len > data.len() {
            return None;
        }
        let s = from_utf16le(&data[offset + 2..offset + 2 + byte_len])?;
        Some((s.trim_end_matches('\0').to_string(), offset + 2 + byte_len))
    } else {
        let char_count = u16::from_le_bytes(data[offset..offset + 2].try_into().ok()?) as usize;
        let byte_len = char_count;
        if offset + 2 + byte_len > data.len() {
            return None;
        }
        let bytes = &data[offset + 2..offset + 2 + byte_len];
        let s = String::from_utf8_lossy(bytes).to_string();
        Some((s.trim_end_matches('\0').to_string(), offset + 2 + byte_len))
    }
}

/// 规范化路径，处理 \\?\ 前缀等
pub fn normalize_path(path: &str) -> String {
    path.replace("\\\\?\\", "").replace("\\??\\", "")
}

/// 获取路径的链接信息
pub fn get_link_info(path: &Path) -> LinkInfo {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => {
            return LinkInfo {
                link_type: LinkType::Unknown,
                target: None,
            };
        }
    };

    // 检查符号链接
    if metadata.file_type().is_symlink() {
        let target = std::fs::read_link(path).ok().map(|p| p.to_string_lossy().to_string());
        return LinkInfo {
            link_type: LinkType::Symlink,
            target,
        };
    }

    // 检查 .lnk
    if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("lnk")) {
        let target = get_lnk_target(path);
        return LinkInfo {
            link_type: LinkType::Shortcut,
            target,
        };
    }

    // 检查 Junction
    if metadata.is_dir() && is_junction(path) {
        let target = get_junction_target(path);
        return LinkInfo {
            link_type: LinkType::Junction,
            target,
        };
    }

    LinkInfo {
        link_type: if metadata.is_dir() { LinkType::Dir } else { LinkType::File },
        target: None,
    }
}

mod kernel32 {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn GetFileAttributesW(lpFileName: *const u16) -> u32;
    }
}
