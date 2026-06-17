use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use crate::links::{self, LinkType};

/// 文件信息
#[derive(Debug, Clone)]
pub struct ItemInfo {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_file: bool,
    pub link_type: LinkType,
    pub link_target: Option<String>,
    pub size: u64,
    pub create_time: Option<SystemTime>,
    pub modify_time: Option<SystemTime>,
}

impl ItemInfo {
    fn from_entry(entry: &fs::DirEntry) -> io::Result<Self> {
        let path = entry.path();
        let name = entry
            .file_name()
            .to_string_lossy()
            .to_string();

        let metadata = entry.metadata()?;
        let file_type = entry.file_type()?;

        let is_symlink = file_type.is_symlink();

        let (link_type, link_target) = if is_symlink {
            let target = fs::read_link(&path).ok().map(|p| p.to_string_lossy().to_string());
            (LinkType::Symlink, target)
        } else if name.to_lowercase().ends_with(".lnk") {
            if file_type.is_file() {
                let target = links::get_lnk_target(&path);
                (LinkType::Shortcut, target)
            } else {
                (LinkType::File, None)
            }
        } else if file_type.is_dir() && links::is_junction(&path) {
            let target = links::get_junction_target(&path);
            (LinkType::Junction, target)
        } else if file_type.is_file() {
            (LinkType::File, None)
        } else if file_type.is_dir() {
            (LinkType::Dir, None)
        } else {
            (LinkType::Unknown, None)
        };

        let size = if file_type.is_file() {
            metadata.len()
        } else {
            0
        };

        let create_time = metadata.created().ok();
        let modify_time = metadata.modified().ok();

        Ok(Self {
            name,
            path,
            is_dir: file_type.is_dir(),
            is_file: file_type.is_file(),
            link_type,
            link_target,
            size,
            create_time,
            modify_time,
        })
    }

    /// 检测是否为 Python 环境目录
    pub fn get_python_env(&self) -> Option<String> {
        if !self.is_dir {
            return None;
        }

        let check_paths = [
            self.path.join("python.exe"),
            self.path.join("Scripts").join("python.exe"),
        ];

        for python_path in &check_paths {
            if python_path.exists() {
                let output = Command::new(python_path)
                    .arg("-V")
                    .output()
                    .ok()?;
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !version.is_empty() {
                        return Some(version);
                    }
                }
            }
        }
        None
    }

    /// 检测是否为 Java 环境目录
    pub fn get_java_env(&self) -> Option<String> {
        if !self.is_dir {
            return None;
        }

        let check_paths = [
            self.path.join("bin").join("java.exe"),
            self.path.join("jre").join("bin").join("java.exe"),
        ];

        for java_path in &check_paths {
            if java_path.exists() {
                let output = Command::new(java_path)
                    .arg("-version")
                    .output()
                    .ok()?;
                if output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if let Some(line) = stderr.lines().next() {
                        if !line.is_empty() {
                            return Some(line.trim().to_string());
                        }
                    }
                }
            }
        }
        None
    }

    /// 获取创建时间时间戳（秒）
    pub fn create_time_secs(&self) -> Option<i64> {
        self.create_time.and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64)
    }

    /// 获取修改时间时间戳（秒）
    pub fn modify_time_secs(&self) -> Option<i64> {
        self.modify_time.and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64)
    }
}

/// 扫描目录，返回 ItemInfo 列表
pub fn scan_directory(path: &Path) -> Vec<ItemInfo> {
    let mut items = Vec::new();
    let entries = match fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => return items,
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        match ItemInfo::from_entry(&entry) {
            Ok(item) => items.push(item),
            Err(_) => continue,
        }
    }

    items
}
