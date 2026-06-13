use std::path::PathBuf;

/// 路径查找策略。
///
/// 按优先级解析数据目录/源目录等路径：
///   - 便携模式（executable 同级优先）
///   - 用户数据目录（`%LOCALAPPDATA%`）
///   - 环境变量覆盖
pub struct PathResolver {
    app_name: &'static str,
}

impl PathResolver {
    /// 创建一个路径解析器。
    pub const fn new(app_name: &'static str) -> Self {
        Self { app_name }
    }

    /// 默认的 aminos 路径解析器。
    pub const fn aminos() -> Self {
        Self { app_name: "aminos" }
    }

    /// 可执行文件同级目录。
    fn sibling_dir(&self, name: &str) -> Option<PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let dir = exe.parent()?.join(name);
        if dir.is_dir() {
            Some(dir)
        } else {
            None
        }
    }

    /// 应用数据根目录：`%LOCALAPPDATA%\{app_name}\`
    fn appdata_root(&self) -> PathBuf {
        let local = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        local.join(self.app_name)
    }

    // ── 数据目录解析 ──

    /// 源定义目录。
    ///
    /// 优先级：
    ///   1. 环境变量 `AMINOS_SOURCE_DIR`
    ///   2. executable 同级 `source/`
    ///   3. `%LOCALAPPDATA%\{app_name}\source\`
    pub fn source_dir(&self) -> PathBuf {
        // 环境变量
        if let Ok(env) = std::env::var("AMINOS_SOURCE_DIR") {
            return PathBuf::from(env);
        }

        // 便携模式
        if let Some(dir) = self.sibling_dir("source") {
            return dir;
        }

        // 用户数据目录
        self.appdata_root().join("source")
    }

    /// 下载缓存目录：`%LOCALAPPDATA%\{app_name}\downloads\`
    pub fn downloads_dir(&self) -> PathBuf {
        self.appdata_root().join("downloads")
    }

    /// 应用记录目录：`%LOCALAPPDATA%\{app_name}\apps\`
    pub fn apps_dir(&self) -> PathBuf {
        self.appdata_root().join("apps")
    }

    /// 安装记录文件路径：`apps\installed.json`
    pub fn installed_json(&self) -> PathBuf {
        self.apps_dir().join("installed.json")
    }

    /// 在资源管理器中打开一个目录。
    pub fn open_in_explorer(path: &std::path::Path) {
        let _ = std::process::Command::new("explorer")
            .arg(path)
            .spawn();
    }
}
