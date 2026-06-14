use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::download::{Cancel, ProgressCtx};

/// 工具二进制目录（由 CLI 在启动时设置）。
static TOOLS_BIN_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 设置工具二进制搜索目录（如 `%LOCALAPPDATA%/aminos/tools/bin`）。
/// 调用方（as CLI）应在下载器命令之前调用。
pub fn set_tools_bin_dir(dir: PathBuf) {
    let _ = TOOLS_BIN_DIR.set(dir);
}

/// 获取工具二进制搜索目录。
pub fn get_tools_bin_dir() -> Option<&'static PathBuf> {
    TOOLS_BIN_DIR.get()
}

/// 操作系统平台。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Platform {
    Windows,
    Linux,
    Macos,
}

impl Platform {
    /// 当前运行的平台。
    pub fn current() -> &'static [Platform] {
        if cfg!(target_os = "windows") {
            &[Platform::Windows]
        } else if cfg!(target_os = "linux") {
            &[Platform::Linux]
        } else if cfg!(target_os = "macos") {
            &[Platform::Macos]
        } else {
            &[]
        }
    }
}

/// 错误严重程度分类。
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorKind {
    /// 瞬时错误：可重试（连接超时、DNS 失败等）
    Transient,
    /// 永久错误：无需重试（404、403、文件损坏等）
    Permanent,
}

/// 分类后的下载错误。
#[derive(Debug, Clone)]
pub struct ClassifiedError {
    pub kind: ErrorKind,
    pub message: String,
}

impl ClassifiedError {
    pub fn transient(msg: impl Into<String>) -> Self {
        Self { kind: ErrorKind::Transient, message: msg.into() }
    }
    pub fn permanent(msg: impl Into<String>) -> Self {
        Self { kind: ErrorKind::Permanent, message: msg.into() }
    }
}

/// 下载后端统一接口。
///
/// 每个下载工具实现此 trait，调度器通过 trait 引用所有后端。
pub trait DownloadBackend: Send + Sync + Debug {
    /// 后端唯一标识（如 "RustRange"）。
    fn id(&self) -> &'static str;

    /// 友好的展示名称。
    fn display_name(&self) -> &'static str {
        self.id()
    }

    /// 支持的平台列表。
    fn supported_platforms(&self) -> &[Platform];

    /// 优先级（数字越小越优先尝试）。
    fn priority(&self) -> u8;

    /// 是否有实时进度条（tracked）。
    fn tracked(&self) -> bool;

    /// 线程标签（"多线程" / "单线程"）。
    fn thread_label(&self) -> &'static str;

    /// 健康检查：后端是否可用（二进制存在、平台匹配等）。
    fn health_check(&self) -> bool;

    /// 执行下载。
    fn download(
        &self,
        url: &str,
        target_path: &Path,
        cancel: &Cancel,
        pb: Option<ProgressCtx>,
    ) -> anyhow::Result<()>;
}

// ── RustRange 后端 ─────────────────────────────────

#[derive(Debug)]
pub struct RustRangeBackend {
    pub threads: u8,
    pub resume: bool,
}

impl Default for RustRangeBackend {
    fn default() -> Self {
        Self { threads: 16, resume: true }
    }
}

impl DownloadBackend for RustRangeBackend {
    fn id(&self) -> &'static str { "RustRange" }
    fn supported_platforms(&self) -> &[Platform] { &[Platform::Windows, Platform::Linux, Platform::Macos] }
    fn priority(&self) -> u8 { 1 }
    fn tracked(&self) -> bool { true }
    fn thread_label(&self) -> &'static str { "多线程" }
    fn health_check(&self) -> bool { true } // 纯 Rust，始终可用

    fn download(&self, url: &str, target_path: &Path, cancel: &Cancel, pb: Option<ProgressCtx>) -> anyhow::Result<()> {
        crate::range::parallel_download(url, target_path, self.threads as usize, self.resume, cancel, pb)
    }
}

// ── Aria2c 后端 ────────────────────────────────────

#[derive(Debug)]
pub struct Aria2cBackend;

impl DownloadBackend for Aria2cBackend {
    fn id(&self) -> &'static str { "aria2c" }
    fn supported_platforms(&self) -> &[Platform] { &[Platform::Windows, Platform::Linux, Platform::Macos] }
    fn priority(&self) -> u8 { 2 }
    fn tracked(&self) -> bool { true }
    fn thread_label(&self) -> &'static str { "多线程" }
    fn health_check(&self) -> bool {
        // aria2c 只能通过 as tool 安装方式使用
        get_tools_bin_dir()
            .map(|dir| dir.join("aria2c.exe").is_file())
            .unwrap_or(false)
    }

    fn download(&self, url: &str, target_path: &Path, cancel: &Cancel, pb: Option<ProgressCtx>) -> anyhow::Result<()> {
        crate::aria2c::try_aria2c_download(url, target_path, cancel, pb)
    }
}

// ── Ureq 后端（含 insecure 和 cookie 回退） ─────────

#[derive(Debug)]
pub struct UreqBackend {
    pub insecure: bool,
}

impl UreqBackend {
    pub fn normal() -> Self { Self { insecure: false } }
    pub fn insecure() -> Self { Self { insecure: true } }
}

impl DownloadBackend for UreqBackend {
    fn id(&self) -> &'static str {
        if self.insecure { "ureq(insecure)" } else { "ureq" }
    }
    fn display_name(&self) -> &'static str {
        if self.insecure { "ureq(ins)" } else { "ureq" }
    }
    fn supported_platforms(&self) -> &[Platform] { &[Platform::Windows, Platform::Linux, Platform::Macos] }
    fn priority(&self) -> u8 {
        if self.insecure { 4 } else { 3 }
    }
    fn tracked(&self) -> bool { true }
    fn thread_label(&self) -> &'static str { "单线程" }
    fn health_check(&self) -> bool { true }

    fn download(&self, url: &str, target_path: &Path, cancel: &Cancel, pb: Option<ProgressCtx>) -> anyhow::Result<()> {
        let agent_cfg = crate::agent::AgentConfig {
            fingerprint: crate::agent::Fingerprint::Chrome120,
            insecure: self.insecure,
            connect_timeout: 15,
            read_timeout: 600,
        };
        crate::download::download_with_ureq(url, target_path, &agent_cfg, cancel, pb)
    }
}

// ── PowerShell 后端 ─────────────────────────────────

#[derive(Debug)]
pub struct PowerShellBackend;

impl DownloadBackend for PowerShellBackend {
    fn id(&self) -> &'static str { "powershell" }
    fn supported_platforms(&self) -> &[Platform] { &[Platform::Windows] }
    fn priority(&self) -> u8 { 5 }
    fn tracked(&self) -> bool { false }
    fn thread_label(&self) -> &'static str { "单线程" }
    fn health_check(&self) -> bool {
        cfg!(target_os = "windows")
    }

    fn download(&self, url: &str, target_path: &Path, cancel: &Cancel, _pb: Option<ProgressCtx>) -> anyhow::Result<()> {
        crate::powershell::try_powershell_download(url, target_path, cancel)
    }
}

// ── PowerShell Invoke-WebRequest 后端 ──────────────

#[derive(Debug)]
pub struct PowerShellInvokeBackend;

impl DownloadBackend for PowerShellInvokeBackend {
    fn id(&self) -> &'static str { "ps-invoke" }
    fn supported_platforms(&self) -> &[Platform] { &[Platform::Windows] }
    fn priority(&self) -> u8 { 6 }
    fn tracked(&self) -> bool { false }
    fn thread_label(&self) -> &'static str { "单线程" }
    fn health_check(&self) -> bool {
        cfg!(target_os = "windows")
    }

    fn download(&self, url: &str, target_path: &Path, cancel: &Cancel, _pb: Option<ProgressCtx>) -> anyhow::Result<()> {
        crate::powershell::try_powershell_invoke(url, target_path, cancel)
    }
}

// ── BITS 后端 ──────────────────────────────────────

#[derive(Debug)]
pub struct BitsBackend;

impl DownloadBackend for BitsBackend {
    fn id(&self) -> &'static str { "bits" }
    fn supported_platforms(&self) -> &[Platform] { &[Platform::Windows] }
    fn priority(&self) -> u8 { 7 }
    fn tracked(&self) -> bool { false }
    fn thread_label(&self) -> &'static str { "单线程" }
    fn health_check(&self) -> bool {
        cfg!(target_os = "windows")
    }

    fn download(&self, url: &str, target_path: &Path, cancel: &Cancel, _pb: Option<ProgressCtx>) -> anyhow::Result<()> {
        crate::powershell::try_bits_transfer(url, target_path, cancel)
    }
}

// ── Curl 后端 ───────────────────────────────────────

#[derive(Debug)]
pub struct CurlBackend;

impl DownloadBackend for CurlBackend {
    fn id(&self) -> &'static str { "curl" }
    fn supported_platforms(&self) -> &[Platform] { &[Platform::Windows, Platform::Linux, Platform::Macos] }
    fn priority(&self) -> u8 { 8 }
    fn tracked(&self) -> bool { false }
    fn thread_label(&self) -> &'static str { "单线程" }
    fn health_check(&self) -> bool { which_available("curl") }

    fn download(&self, url: &str, target_path: &Path, cancel: &Cancel, _pb: Option<ProgressCtx>) -> anyhow::Result<()> {
        crate::curl::try_curl_download(url, target_path, cancel)
    }
}

// ── 后端注册表 ─────────────────────────────────────

/// 后端注册表：管理所有注册的后端，提供健康检查和优先级排序。
#[derive(Debug)]
pub struct BackendRegistry {
    backends: Vec<Box<dyn DownloadBackend>>,
}

impl Default for BackendRegistry {
    fn default() -> Self {
        Self::with_default_backends()
    }
}

impl BackendRegistry {
    /// 创建默认后端列表（按优先级排序）。
    pub fn with_default_backends() -> Self {
        let mut backends: Vec<Box<dyn DownloadBackend>> = vec![
            Box::new(RustRangeBackend::default()),
            Box::new(Aria2cBackend),
            Box::new(UreqBackend::normal()),
            Box::new(UreqBackend::insecure()),
            Box::new(PowerShellBackend),
            Box::new(PowerShellInvokeBackend),
            Box::new(BitsBackend),
            Box::new(CurlBackend),
        ];
        backends.sort_by_key(|b| b.priority());
        Self { backends }
    }

    /// 从指定后端列表创建注册表。
    pub fn new(backends: Vec<Box<dyn DownloadBackend>>) -> Self {
        let mut backends = backends;
        backends.sort_by_key(|b| b.priority());
        Self { backends }
    }

    /// 返回所有后端（按优先级排序）。
    pub fn all(&self) -> &[Box<dyn DownloadBackend>] {
        &self.backends
    }

    /// 返回当前平台可用的后端（health_check 通过）。
    pub fn available(&self) -> Vec<&Box<dyn DownloadBackend>> {
        let current = Platform::current();
        self.backends.iter()
            .filter(|b| {
                b.supported_platforms().iter().any(|p| current.contains(p))
                    && b.health_check()
            })
            .collect()
    }

    /// 通过 id 查找后端。
    pub fn by_id(&self, id: &str) -> Option<&Box<dyn DownloadBackend>> {
        self.backends.iter().find(|b| b.id() == id)
    }

    /// 按 tracked/untracked 分组（tracked 优先用于进度条）。
    pub fn ordered(&self) -> Vec<&Box<dyn DownloadBackend>> {
        let current = Platform::current();
        let mut tracked: Vec<_> = self.backends.iter()
            .filter(|b| b.tracked() && b.supported_platforms().iter().any(|p| current.contains(p)))
            .collect();
        let mut untracked: Vec<_> = self.backends.iter()
            .filter(|b| !b.tracked() && b.supported_platforms().iter().any(|p| current.contains(p)))
            .collect();
        tracked.sort_by_key(|b| b.priority());
        untracked.sort_by_key(|b| b.priority());
        tracked.into_iter().chain(untracked).collect()
    }
}

// ── 工具函数 ────────────────────────────────────────

/// 检查 PATH 上是否存在指定二进制。
fn which_available(name: &str) -> bool {
    which_path(name).is_some()
}

/// 在 PATH 中查找指定二进制文件的完整路径。
fn which_path(name: &str) -> Option<String> {
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path).find_map(|dir| {
            let exe = if cfg!(windows) {
                dir.join(format!("{}.exe", name))
            } else {
                dir.join(name)
            };
            if exe.is_file() {
                exe.to_str().map(|s| s.to_string())
            } else {
                None
            }
        })
    })
}

// ── 后端元信息查询（用于 `as downloader list`） ──

/// 该后端是否为纯内置实现（无外部二进制依赖）。
pub fn backend_is_builtin(name: &str) -> bool {
    matches!(name, "RustRange" | "Ureq" | "UreqInsecure")
}

/// 查询指定后端是否处于可用状态（health_check 通过）。
pub fn backend_is_available(name: &str) -> bool {
    match name {
        "RustRange" => true, // 纯 Rust
        "Aria2c" => get_tools_bin_dir()
            .map(|dir| dir.join("aria2c.exe").is_file())
            .unwrap_or(false),
        "Ureq" | "UreqInsecure" => true,
        "PowerShell" | "PowerShellInvoke" | "BitsTransfer" => {
            which_path("powershell").is_some()
                || std::path::Path::new("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe").is_file()
        }
        "Curl" => which_path("curl").is_some()
            || std::path::Path::new("C:\\Windows\\System32\\curl.exe").is_file(),
        _ => false,
    }
}

/// 查询指定后端名称对应的二进制路径（如有）。
pub fn backend_binary_path(name: &str) -> Option<String> {
    if backend_is_builtin(name) {
        return None; // 纯内置，无二进制
    }
    match name {
        "Aria2c" => {
            get_tools_bin_dir()
                .map(|dir| dir.join("aria2c.exe"))
                .filter(|p| p.is_file())
                .and_then(|p| p.to_str().map(|s| s.to_string()))
        }
        "PowerShell" | "PowerShellInvoke" | "BitsTransfer" => {
            which_path("powershell")
                .or_else(|| Some("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe".into()))
        }
        "Curl" => which_path("curl"),
        _ => None,
    }
}

/// 查询指定后端是否支持 Range 分片下载。
pub fn backend_supports_range(name: &str) -> bool {
    matches!(name, "RustRange" | "Aria2c" | "Curl" | "BitsTransfer")
}
