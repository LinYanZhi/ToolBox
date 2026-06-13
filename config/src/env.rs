use std::path::PathBuf;

/// 环境变量统一管理。
///
/// 所有 aminos 相关的环境变量以 `AMINOS_` 为前缀，提供命名空间化查询。
///
/// # 优先级
/// CLI 参数 > 环境变量 > 配置文件 > 内置默认值
pub struct EnvConfig {
    namespace: &'static str,
}

impl EnvConfig {
    /// 用指定命名空间创建配置查询器。
    pub const fn new(namespace: &'static str) -> Self {
        Self { namespace }
    }

    /// 默认的 aminos 命名空间。
    pub const fn aminos() -> Self {
        Self { namespace: "AMINOS" }
    }

    /// 构建完整的键名：`{NAMESPACE}_{KEY}`。
    fn key(&self, suffix: &str) -> String {
        if suffix.is_empty() {
            self.namespace.to_string()
        } else {
            format!("{}_{}", self.namespace, suffix)
        }
    }

    /// 获取环境变量值（可选）。
    pub fn get(&self, key: &str) -> Option<String> {
        std::env::var(self.key(key)).ok()
    }

    /// 获取环境变量值，不存在则返回默认值。
    pub fn get_or(&self, key: &str, default: &str) -> String {
        self.get(key).unwrap_or_else(|| default.to_string())
    }

    /// 获取环境变量解析为路径（可选）。
    pub fn get_path(&self, key: &str) -> Option<PathBuf> {
        self.get(key).map(PathBuf::from)
    }

    /// 获取环境变量解析为 u64。
    pub fn get_u64(&self, key: &str) -> Option<u64> {
        self.get(key)?.parse().ok()
    }

    // ── 命名环境变量访问器（方便调用方，不强制使用）──

    pub fn source_repo(&self) -> Option<String> {
        self.get("SOURCE_REPO")
    }

    pub fn source_dir(&self) -> Option<PathBuf> {
        self.get_path("SOURCE_DIR")
    }

    pub fn download_dir(&self) -> Option<PathBuf> {
        self.get_path("DOWNLOAD_DIR")
    }

    pub fn aria2c_path(&self) -> Option<PathBuf> {
        self.get_path("ARIA2C_PATH")
    }

    pub fn proxy(&self) -> Option<String> {
        self.get("PROXY")
    }

    pub fn max_threads(&self) -> Option<u64> {
        self.get_u64("MAX_THREADS")
    }

    pub fn insecure(&self) -> bool {
        self.get("INSECURE").map(|v| v == "1" || v == "true").unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_format() {
        let cfg = EnvConfig::aminos();
        assert_eq!(cfg.key("SOURCE_REPO"), "AMINOS_SOURCE_REPO");
        assert_eq!(cfg.key(""), "AMINOS");
    }
}
