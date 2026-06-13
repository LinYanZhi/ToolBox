use std::io::Read;
use std::path::Path;

/// 校验下载文件的签名是否合法。
///
/// 先检查文件大小，再读取前 4KB 检查魔数，按扩展名分类验证。
/// 这是轻量级的完整性检查，用于防止反盗链页面替代真实文件。
pub fn verify_downloaded_file(path: &Path) -> bool {
    let fname = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    let file_size = match std::fs::metadata(path) {
        Ok(m) => m.len(),
        _ => return false,
    };

    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        _ => return false,
    };
    let mut header = [0u8; 4096];
    let n = match file.read(&mut header) {
        Ok(n) if n >= 4 => n,
        _ => return false,
    };

    // 按扩展名检查文件魔数和最小大小
    if fname.ends_with(".exe") || fname.ends_with(".dll") || fname.ends_with(".msi") {
        // PE 文件: MZ 开头 (4D 5A)，且大于 512KB（防反盗链页面）
        header[0] == 0x4D && header[1] == 0x5A && file_size >= 512 * 1024
    } else if fname.ends_with(".zip") || fname.ends_with(".7z") {
        (header[0] == 0x50 && header[1] == 0x4B && header[2] == 0x03 && header[3] == 0x04)
            || (header[0] == 0x37 && header[1] == 0x7A)
    } else if fname.ends_with(".rar") {
        header[0] == 0x52 && header[1] == 0x61 && header[2] == 0x72 && header[3] == 0x21
    } else if fname.ends_with(".tar") {
        n > 1024
    } else if fname.ends_with(".gz") || fname.ends_with(".xz") || fname.ends_with(".bz2") {
        (header[0] == 0x1F && header[1] == 0x8B)
            || (header[0] == 0xFD && header[1] == 0x37)
            || (header[0] == 0x42 && header[1] == 0x5A)
    } else if fname.ends_with(".iso") {
        header[0] == 0x43 && header[1] == 0x44 && header[2] == 0x30 && file_size >= 1024 * 1024
    } else if fname.ends_with(".appx") || fname.ends_with(".msix") {
        header[0] == 0x50 && header[1] == 0x4B && header[2] == 0x03 && header[3] == 0x04
    } else {
        n > 1024
    }
}
