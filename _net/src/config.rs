use crate::backend::*;

/// 下载配置 — 所有后端硬编码默认启用，无需用户管理。
#[derive(Debug)]
pub struct DownloaderConfig {
    /// 最大并发数。
    pub max_concurrent: usize,
    /// 是否校验下载文件。
    pub verify: bool,
    /// 是否启用续传。
    pub resume: bool,
    /// 后端列表（按优先级排序）。
    pub backends: Vec<Box<dyn DownloadBackend>>,
}

impl Default for DownloaderConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 16,
            verify: true,
            resume: true,
            backends: default_backends(),
        }
    }
}

impl DownloaderConfig {
    /// 加载配置 — 始终返回默认硬编码配置，不读取任何文件。
    pub fn load() -> Self {
        Self::default()
    }
}

fn default_backends() -> Vec<Box<dyn DownloadBackend>> {
    vec![
        Box::new(Aria2cBackend),
        Box::new(RustRangeBackend::default()),
        Box::new(UreqBackend::normal()),
        Box::new(UreqBackend::insecure()),
        Box::new(PowerShellBackend),
        Box::new(PowerShellInvokeBackend),
        Box::new(BitsBackend),
        Box::new(CurlBackend),
    ]
}

// ── 后向兼容（旧 aminos crate 还引用这些函数） ──────────

/// 列出后端及其启用状态 — 始终返回全部启用的后端列表，不读取配置文件。
pub fn list_backend_states() -> Vec<(String, bool, Option<u8>)> {
    vec![
        ("aria2c".into(), true, None),
        ("rust-range".into(), true, Some(16)),
        ("rust-ureq".into(), true, None),
        ("rust-ureq(insecure)".into(), true, None),
        ("powershell".into(), true, None),
        ("ps-invoke".into(), true, None),
        ("bits".into(), true, None),
        ("curl".into(), true, None),
    ]
}

/// 返回模拟的配置文件路径（实际上不再创建文件）。
pub fn config_file_path() -> std::path::PathBuf {
    let local = std::env::var("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    local.join("aminos").join("config").join("downloader.toml")
}

/// 修改后端启用状态（旧接口兼容 — 不再实际写入配置文件）。
pub fn set_backend_enabled(name: &str, enabled: bool) -> anyhow::Result<()> {
    // 不再创建或修改配置文件，仅做名称校验
    let all = list_backend_states();
    if !all.iter().any(|(n, _, _)| n.eq_ignore_ascii_case(name)) {
        let names: Vec<&str> = all.iter().map(|(n, _, _)| n.as_str()).collect();
        anyhow::bail!("未找到后端: {}（可用后端: {}）", name, names.join(", "));
    }
    if enabled {
        eprintln!("  {} 已默认启用，无需操作", name);
    } else {
        eprintln!("  不再支持关闭内置后端，{} 将保持启用", name);
    }
    Ok(())
}

/// 查找后端标准名称（旧接口兼容）。
pub fn find_backend_name(name: &str) -> String {
    let all = list_backend_states();
    for (n, _, _) in &all {
        if n.eq_ignore_ascii_case(name) {
            return n.clone();
        }
    }
    name.to_string()
}
