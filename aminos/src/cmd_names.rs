//! 集中管理所有外部显示的命令名称和提示文本。
//!
//! ⚠️  所有提示用户「运行什么命令」的文本，都必须使用这里的常量。
//!    严禁在代码中硬编码 `"as xxx"` 字符串。
//!    如需修改命令名称/格式，只改此文件即可。

// ── 子命令名称 ─────────────────────────────────────

pub const INSTALL:     &str = "as install";
pub const INFO:        &str = "as info";
pub const LIST:        &str = "as list";
pub const DOWNLOAD:    &str = "as download";
pub const UNINSTALL:   &str = "as uninstall";
pub const CACHE:       &str = "as cache";
pub const SOURCE:      &str = "as source";


// ── 子命令 + 子操作 ─────────────────────────────────

pub const SOURCE_UPDATE:         &str = "as source update";
pub const SOURCE_CLEAR:          &str = "as source -c";
pub const SOURCE_OPEN:           &str = "as source -o";
pub const SOURCE_ADD:            &str = "as source add";
pub const SOURCE_SPEEDTEST:      &str = "as source --speedtest";
pub const DOWNLOADER_OPEN:       &str = "as downloader -o";
pub const DOWNLOADER_LIST:       &str = "as downloader --list";
pub const DOWNLOADER_SET:        &str = "as downloader set";
pub const TOOL_INIT:             &str = "as tool init";
pub const TOOL_ADD:              &str = "as tool add";
pub const TOOL_LIST:             &str = "as tool list";
pub const TOOL_REMOVE:           &str = "as tool remove";
pub const CACHE_CLEAR:           &str = "as cache -c";
pub const CACHE_OPEN:            &str = "as cache -o";

// ── 别名（向下兼容） ────────────────────────────────

pub const SOURCE_UPDATE_HINT: &str = SOURCE_UPDATE;
