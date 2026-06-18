/// 大小阈值规则
#[derive(Debug, Clone)]
pub struct SizeRule {
    pub max: i64,
    pub color: String,
    pub mode: String,
}

/// 运行时颜色配置（代码内硬编码，无外部配置文件）
pub struct ColorConfig {
    /// 普通目录颜色
    pub dir_color: String,
    /// 链接目录颜色
    pub dir_link_color: String,
    /// 目录链接箭头
    pub dir_link_arrow: String,
    /// 目录链接箭头颜色
    pub dir_link_arrow_color: String,
    /// 目录链接路径颜色
    pub dir_link_path_color: String,
    /// 目录链接路径中的目录名颜色
    pub dir_link_path_basename_color: String,

    /// 文件扩展名 → 颜色
    pub file_extensions: Vec<(String, String)>,
    /// 文件链接箭头
    pub file_link_arrow: String,
    /// 文件链接箭头颜色
    pub file_link_arrow_color: String,
    /// 文件链接指向目录时的目录名颜色
    pub file_link_dir_color: String,

    /// 文件大小颜色规则
    pub size_rules: Vec<SizeRule>,
}

impl ColorConfig {
    /// 初始化 ANSI 支持
    pub fn init() {
        color::enable_ansi();
    }

    /// 加载配置（直接使用代码内硬编码默认值）
    pub fn load() -> Self {
        ColorConfig::default()
    }

    /// 根据扩展名获取颜色
    pub fn ext_color(&self, ext: &str) -> Option<&str> {
        let ext_lower = ext.to_lowercase();
        for (pattern, color) in &self.file_extensions {
            if pattern == &ext_lower {
                return Some(color);
            }
        }
        None
    }
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            dir_color: "94".into(),
            dir_link_color: "94".into(),
            dir_link_arrow: "=>".into(),
            dir_link_arrow_color: "90".into(),
            dir_link_path_color: "90".into(),
            dir_link_path_basename_color: "94".into(),

            file_extensions: vec![
                (".7z".into(),   "31".into()),
                (".zip".into(),  "31".into()),
                (".rar".into(),  "31".into()),
                (".tar".into(),  "31".into()),
                (".gz".into(),   "31".into()),
                (".bz2".into(),  "31".into()),
                (".xz".into(),   "31".into()),
                (".exe".into(),  "32".into()),
                (".msi".into(),  "32".into()),
                (".bat".into(),  "32".into()),
                (".cmd".into(),  "32".into()),
                (".py".into(),   "93".into()),
                (".rs".into(),   "33".into()),
                (".js".into(),   "33".into()),
                (".ts".into(),   "33".into()),
                (".html".into(), "35".into()),
                (".css".into(),  "35".into()),
                (".json".into(), "37".into()),
                (".toml".into(), "37".into()),
                (".yaml".into(), "37".into()),
                (".yml".into(),  "37".into()),
                (".md".into(),   "37".into()),
                (".txt".into(),  "37".into()),
                (".lnk".into(),  "94".into()),
                (".dll".into(),  "90".into()),
                (".pdb".into(),  "90".into()),
                (".dat".into(),  "90".into()),
                (".ini".into(),  "90".into()),
                (".lock".into(), "90".into()),
                (".log".into(),  "90".into()),
            ],
            file_link_arrow: "->".into(),
            file_link_arrow_color: "90".into(),
            file_link_dir_color: "94".into(),

            size_rules: vec![
                SizeRule { max: 1024, color: "90".into(), mode: "full".into() },
                SizeRule { max: 1_048_576, color: "90".into(), mode: "unit".into() },
                SizeRule { max: 104_857_600, color: "93".into(), mode: "unit".into() },
                SizeRule { max: 1_073_741_824, color: "93".into(), mode: "full".into() },
                SizeRule { max: 2_147_483_648, color: "91".into(), mode: "unit".into() },
                SizeRule { max: -1, color: "91".into(), mode: "full".into() },
            ],
        }
    }
}
