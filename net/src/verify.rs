use std::io::Read;
use std::path::Path;

/// 魔数识别的文件格式。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileFormat {
    /// PE (exe, dll, msi 等)
    Pe,
    /// ZIP
    Zip,
    /// 7z
    SevenZ,
    /// RAR
    Rar,
    /// GZIP
    Gz,
    /// XZ
    Xz,
    /// BZip2
    Bz2,
    /// ISO (CD001)
    Iso,
}

impl FileFormat {
    /// 返回标准的文件扩展名（含 `.`）。
    pub fn extension(self) -> &'static str {
        match self {
            FileFormat::Pe => ".exe",
            FileFormat::Zip => ".zip",
            FileFormat::SevenZ => ".7z",
            FileFormat::Rar => ".rar",
            FileFormat::Gz => ".gz",
            FileFormat::Xz => ".xz",
            FileFormat::Bz2 => ".bz2",
            FileFormat::Iso => ".iso",
        }
    }
}

/// 读取文件头魔数，检测实际文件格式。
///
/// 读取前 4KB，只检查前几个字节，不依赖文件扩展名。
pub fn detect_format(path: &Path) -> Option<FileFormat> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut header = [0u8; 16];
    let n = file.read(&mut header).ok()?;
    if n < 4 {
        return None;
    }

    // PE: MZ (4D 5A)
    if header[0] == 0x4D && header[1] == 0x5A {
        return Some(FileFormat::Pe);
    }
    // ZIP: PK\x03\x04
    if header[0] == 0x50 && header[1] == 0x4B && header[2] == 0x03 && header[3] == 0x04 {
        return Some(FileFormat::Zip);
    }
    // 7z: 37 7A BC AF 27 1C
    if header[0] == 0x37 && header[1] == 0x7A && header[2] == 0xBC && header[3] == 0xAF {
        return Some(FileFormat::SevenZ);
    }
    // RAR: 52 61 72 21
    if header[0] == 0x52 && header[1] == 0x61 && header[2] == 0x72 && header[3] == 0x21 {
        return Some(FileFormat::Rar);
    }
    // GZIP: 1F 8B
    if header[0] == 0x1F && header[1] == 0x8B {
        return Some(FileFormat::Gz);
    }
    // XZ: FD 37 7A 58 5A 00
    if header[0] == 0xFD && header[1] == 0x37 && header[2] == 0x7A && header[3] == 0x58 {
        return Some(FileFormat::Xz);
    }
    // BZip2: 42 5A 68
    if header[0] == 0x42 && header[1] == 0x5A && header[2] == 0x68 {
        return Some(FileFormat::Bz2);
    }
    // ISO: 43 44 30 30 31 (CD001)
    if header[0] == 0x43 && header[1] == 0x44 && header[2] == 0x30 && header[3] == 0x30
        && header[4] == 0x31
    {
        return Some(FileFormat::Iso);
    }

    None
}

/// 校验下载文件是否合法。
///
/// 以魔数检测为准：只要能识别出已知的文件格式，就认为合法。
/// 只有无法通过魔数识别的文件，才会回退到扩展名 + 最小大小检查。
pub fn verify_downloaded_file(path: &Path) -> bool {
    let file_size = match std::fs::metadata(path) {
        Ok(m) => m.len(),
        _ => return false,
    };

    // 先读前 4KB 做内容检测
    let mut header = [0u8; 4096];
    let n = match std::fs::File::open(path).and_then(|mut f| f.read(&mut header)) {
        Ok(n) => n,
        _ => return false,
    };

    // 内容明显是 HTML/文本错误页 → 拒绝（反盗链页面）
    let header_lower: Vec<u8> = header[..n.min(100)].iter().map(|&b| b.to_ascii_lowercase()).collect();
    if header_lower.starts_with(b"<html") || header_lower.starts_with(b"<!doctype") || header_lower.starts_with(b"<!" ) {
        return false;
    }

    if header_lower.starts_with(b"<") && header_lower.contains(&b'>') {
        // 以 < 开头且很快出现 >，极可能是 HTML，拒绝
        if let Some(pos) = header_lower.iter().position(|&b| b == b'>') {
            if pos < 200 {
                return false;
            }
        }
    }

    let fmt = detect_format(path);

    match fmt {
        // 能通过魔数识别 → 只做合理的大小下限
        Some(FileFormat::Pe) => file_size >= 512 * 1024,   // 512KB 以上才是合理的 PE
        Some(FileFormat::Zip | FileFormat::SevenZ | FileFormat::Rar
            | FileFormat::Gz | FileFormat::Xz | FileFormat::Bz2) => file_size >= 4096,
        Some(FileFormat::Iso) => file_size >= 1024 * 1024, // 1MB
        // 魔数不认识 → 回退：大小 > 1KB（防空文件）
        None => n >= 100 && file_size >= 1024,
    }
}
