use std::path::PathBuf;

/// 系统工具枚举。
///
/// 提供统一的多优先级查找策略：
///   1. 环境变量（如 `AMINOS_ARIA2C_PATH`）
///   2. `%LOCALAPPDATA%\aminos\tools\{name}\`（as 工具包管理）
///   3. executable 同级目录（便携模式）
///   4. `%USERPROFILE%\Desktop`（方便测试）
///   5. PATH 环境变量
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    Aria2c,
    Curl,
    SevenZip,
}

impl Tool {
    /// 工具的可执行文件名。
    pub fn exe_name(&self) -> &'static str {
        match self {
            Tool::Aria2c => "aria2c.exe",
            Tool::Curl => "curl.exe",
            Tool::SevenZip => "7z.exe",
        }
    }

    /// 对应的环境变量名。
    pub fn env_var(&self) -> &'static str {
        match self {
            Tool::Aria2c => "AMINOS_ARIA2C_PATH",
            Tool::Curl => "AMINOS_CURL_PATH",
            Tool::SevenZip => "AMINOS_7Z_PATH",
        }
    }

    /// 按优先级查找工具位置。
    pub fn find(&self) -> Option<PathBuf> {
        // 1. 环境变量显式指定
        if let Ok(path) = std::env::var(self.env_var()) {
            let p = PathBuf::from(path);
            if p.is_file() {
                return Some(p);
            }
        }

        // 2. as 工具包目录：%LOCALAPPDATA%\aminos\tools\{name}\{name}.exe
        if let Some(localappdata) = std::env::var_os("LOCALAPPDATA") {
            let candidate = PathBuf::from(localappdata)
                .join("aminos")
                .join("tools")
                .join(self.exe_name().trim_end_matches(".exe"))
                .join(self.exe_name());
            if candidate.is_file() {
                return Some(candidate);
            }
        }

        // 3. executable 同级目录（便携模式向后兼容）
        if let Some(parent) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()))
        {
            let candidate = parent.join(self.exe_name());
            if candidate.is_file() {
                return Some(candidate);
            }
        }

        // 4. 桌面（方便测试）
        if let Some(desktop) = Self::desktop_dir() {
            let candidate = desktop.join(self.exe_name());
            if candidate.is_file() {
                return Some(candidate);
            }
        }

        // 5. PATH 环境变量
        Self::find_in_path(self.exe_name())
    }

    fn find_in_path(name: &str) -> Option<PathBuf> {
        std::env::var_os("PATH").and_then(|paths| {
            for dir in std::env::split_paths(&paths) {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
            None
        })
    }

    fn desktop_dir() -> Option<PathBuf> {
        let userprofile = std::env::var_os("USERPROFILE")?;
        Some(PathBuf::from(userprofile).join("Desktop"))
    }

    /// 列出所有工具及其状态。
    pub fn list_all() -> Vec<(Tool, Option<PathBuf>)> {
        vec![
            (Tool::Aria2c, Tool::Aria2c.find()),
            (Tool::Curl, Tool::Curl.find()),
            (Tool::SevenZip, Tool::SevenZip.find()),
        ]
    }
}
