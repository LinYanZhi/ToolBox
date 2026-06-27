use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

/// 读取 PE 文件的版本信息（右键 → 属性 → 详细信息 → 文件版本）
///
/// 内部调用 Win32 API：GetFileVersionInfoSizeW → GetFileVersionInfoW → VerQueryValueW
///
/// 先尝试从 StringFileInfo 读取"FileVersion"（如 "26.01"），
/// 失败则从 VS_FIXEDFILEINFO 读取固定版本号（如 "26.1.0.0"）。
pub fn get_pe_version(path: &Path) -> Option<String> {
    // 只对 PE 文件尝试
    let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if !fname.ends_with(".exe") && !fname.ends_with(".dll") && !fname.ends_with(".msi") {
        return None;
    }

    // 转为以 \0 结尾的宽字符串
    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // Safety: 以下 FFI 调用均遵循 Win32 API 规范：
    // - `wide.as_ptr()` 以 \0 结尾，符合 Win32 路径参数要求
    // - `GetFileVersionInfoSizeW` 返回值 size 保证后续 buffer 足够
    // - `GetFileVersionInfoW` 填充的 buffer 生命周期覆盖所有 VerQueryValueW 调用
    // - `VerQueryValueW` 返回的指针生命周期绑定于 buffer，在 unsafe 块内有效
    // - trans_ptr/ver_ptr/fixed_ptr 指向的内存由 VerQueryValueW 从 buffer 中派生，
    //   不拥有所有权，不需要释放
    unsafe {
        // 1. 获取版本信息大小
        let size = GetFileVersionInfoSizeW(wide.as_ptr(), std::ptr::null_mut());
        if size == 0 {
            return None;
        }

        // 2. 分配缓冲区并获取版本信息
        let mut buf: Vec<u8> = vec![0u8; size as usize];
        let ptr = buf.as_mut_ptr() as *mut std::ffi::c_void;
        if GetFileVersionInfoW(wide.as_ptr(), 0, size, ptr) == 0 {
            return None;
        }

        // 3. 先尝试从 StringFileInfo 读取 FileVersion
        // 3a. 获取语言代码页
        let mut trans_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut trans_len: u32 = 0;
        let trans_key = wstr("\\VarFileInfo\\Translation");

        // Safety: ptr 指向 GetFileVersionInfoW 填充的有效 buffer，
        // trans_key 是 \0 结尾的宽字符串
        if VerQueryValueW(ptr, trans_key.as_ptr(), &mut trans_ptr, &mut trans_len) != 0
            && trans_len >= 4
        {
            // Safety: trans_len >= 4 保证 trans_ptr 至少包含一个 u32，
            // trans_ptr 指向 buffer 内有效内存
            let lang_cp = *(trans_ptr as *const u16) as u32
                | ((*(trans_ptr as *const u16).add(1) as u32) << 16);

            // 3b. 查询 StringFileInfo\LangCP\FileVersion
            let ver_key = format!(
                "\\StringFileInfo\\{:04X}{:04X}\\FileVersion",
                lang_cp & 0xFFFF,
                (lang_cp >> 16) & 0xFFFF,
            );
            let ver_key_wide = wstr(&ver_key);

            let mut ver_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let mut ver_len: u32 = 0;
            // Safety: ver_key_wide 是 \0 结尾的宽字符串
            if VerQueryValueW(ptr, ver_key_wide.as_ptr(), &mut ver_ptr, &mut ver_len) != 0
                && ver_len > 0
            {
                // Safety: ver_len 是以字节为单位的长度，ver_ptr 是有效对齐的 u16 指针
                let slice = std::slice::from_raw_parts(ver_ptr as *const u16, (ver_len / 2) as usize);
                // 去掉末尾空字符
                let end = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
                if let Ok(s) = String::from_utf16(&slice[..end]) {
                    let trimmed = s.trim().to_string();
                    if !trimmed.is_empty() {
                        return Some(trimmed);
                    }
                }
            }
        }

        // 4. 回退：从 VS_FIXEDFILEINFO 读取固定版本号
        let mut fixed_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut fixed_len: u32 = 0;
        let root_key = wstr("\\");

        // Safety: root_key 是 \0 结尾的宽字符串 "\\"
        if VerQueryValueW(ptr, root_key.as_ptr(), &mut fixed_ptr, &mut fixed_len) != 0
            && fixed_len >= std::mem::size_of::<VS_FIXEDFILEINFO>() as u32
        {
            // Safety: fixed_len >= size_of<VS_FIXEDFILEINFO> 保证指针指向完整的结构体
            let info = &*(fixed_ptr as *const VS_FIXEDFILEINFO);
            let major = (info.dwFileVersionMS >> 16) & 0xFFFF;
            let minor = info.dwFileVersionMS & 0xFFFF;
            let patch = (info.dwFileVersionLS >> 16) & 0xFFFF;
            let build = info.dwFileVersionLS & 0xFFFF;
            let version = if build == 0 {
                format!("{}.{}.{}", major, minor, patch)
            } else {
                format!("{}.{}.{}.{}", major, minor, patch, build)
            };
            return Some(version);
        }

        None
    }
}

// ── Win32 FFI ─────────────────────────────────────────────

#[allow(non_snake_case)]
#[repr(C)]
struct VS_FIXEDFILEINFO {
    dwSignature: u32,
    dwStrucVersion: u32,
    dwFileVersionMS: u32,
    dwFileVersionLS: u32,
    dwProductVersionMS: u32,
    dwProductVersionLS: u32,
    dwFileFlagsMask: u32,
    dwFileFlags: u32,
    dwFileOS: u32,
    dwFileType: u32,
    dwFileSubtype: u32,
    dwFileDateMS: u32,
    dwFileDateLS: u32,
}

// Safety: link("version") 链接到 Windows 的 version.dll，这些声明是标准 Win32 API，
// 参数类型匹配 MSDN 文档定义，调用方保证传递有效的指针和长度。
#[link(name = "version")]
unsafe extern "system" {
    fn GetFileVersionInfoSizeW(
        lptstrFilename: *const u16,
        dwHandle: *mut u32,
    ) -> u32;

    fn GetFileVersionInfoW(
        lptstrFilename: *const u16,
        dwHandle: u32,
        dwLen: u32,
        lpData: *mut std::ffi::c_void,
    ) -> i32;

    fn VerQueryValueW(
        pBlock: *const std::ffi::c_void,
        lpSubBlock: *const u16,
        lplpBuffer: *mut *mut std::ffi::c_void,
        puLen: *mut u32,
    ) -> i32;
}

// ── 辅助函数 ──────────────────────────────────────────────

/// 将 Rust 字符串转为以 `\0` 结尾的宽字符向量
fn wstr(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
