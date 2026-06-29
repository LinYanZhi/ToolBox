// ── 仓库统一配置 ───────────────────────────────────────────
//
// 所有与远端仓库相关的 URL 和标识符在此统一管理，
// 修改仓库地址只需改这一个文件。

#![allow(dead_code)]

/// 源定义仓库（GitHub `owner/repo` 格式）。
pub const SOURCE_REPO: &str = "LinYanZhi/aminos-source";

/// 源定义仓库的 raw 下载前缀（`/main` 分支）。
pub const SOURCE_RAW_URL: &str =
    "https://raw.githubusercontent.com/LinYanZhi/aminos-source/main";

/// 源定义仓库的 GitHub 页面（供用户提交 PR）。
pub const SOURCE_GITHUB_URL: &str =
    "https://github.com/LinYanZhi/aminos-source";

/// 当前项目（ToolBox）的 GitHub 仓库。
pub const PROJECT_REPO: &str = "LinYanZhi/ToolBox";

/// 当前项目的 GitHub 页面。
pub const PROJECT_URL: &str =
    "https://github.com/LinYanZhi/ToolBox";
