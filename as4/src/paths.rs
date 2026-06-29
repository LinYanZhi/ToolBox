use std::path::PathBuf;

pub fn as_dir() -> PathBuf {
    let exe = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("as.exe"));
    exe.parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}