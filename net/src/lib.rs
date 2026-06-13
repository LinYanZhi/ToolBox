pub mod agent;
pub mod aria2c;
pub mod curl;
pub mod download;
pub mod powershell;
pub mod range;
pub mod speedtest;
pub mod verify;

pub use agent::{AgentConfig, Fingerprint};
pub use download::{download_with_fallback, probe_filename, DownloadConfig, DownloadReport, DownloadStrategy};
pub use range::parallel_download;
pub use verify::{detect_format, verify_downloaded_file, FileFormat};

/// 格式化字节数为人类可读字符串（代理到 color crate）。
pub fn format_size(size: u64) -> String {
    color::format_size(size)
}
