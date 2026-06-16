//! 集中管理所有外部显示的命令名称和提示文本。
//!
//! 改名或重组织命令结构时，只需修改此文件一处。
//! 所有外部输出的 `as xxx` 提示文本都必须引用这里的常量。

// ── 根级命令 ──────────────────────────────────

pub const INSTALL:  &str = "as install";
pub const LIST:     &str = "as list";
pub const UNINSTALL:&str = "as uninstall";
pub const UPGRADE:  &str = "as upgrade";
pub const CONFIG:   &str = "as config";
pub const TOOL:     &str = "as tool";

// ── config 子命令 ──────────────────────────────

pub const CONFIG_PATH:        &str = "as config path";
pub const CONFIG_CACHE:       &str = "as config cache";
pub const CONFIG_CACHE_CLEAR: &str = "as config cache --clear";
pub const CONFIG_CACHE_OPEN:  &str = "as config cache --open";
pub const CONFIG_SOURCE:      &str = "as config source";
pub const CONFIG_SOURCE_UPDATE: &str = "as config source update";
pub const CONFIG_SOURCE_PATH: &str = "as config source path";
pub const CONFIG_SPEEDTEST:   &str = "as config speedtest";
pub const CONFIG_DOWNLOADER:  &str = "as config downloader";
pub const CONFIG_DOWNLOADER_LIST:  &str = "as config downloader list";
pub const CONFIG_DOWNLOADER_CONFIG: &str = "as config downloader config";
pub const CONFIG_DOWNLOADER_CONFIG_OPEN: &str = "as config downloader config --open";

// ── tool 子命令 ────────────────────────────────

pub const TOOL_INIT:    &str = "as tool init";
pub const TOOL_INSTALL: &str = "as tool install";
pub const TOOL_REMOVE:  &str = "as tool remove";
pub const TOOL_LIST:    &str = "as tool list";
pub const TOOL_UPGRADE: &str = "as tool upgrade";

// ── 快捷的通用提示 ─────────────────────────────

pub const SOURCE_UPDATE_HINT: &str = "as config source update";
pub const DOWNLOADER_SET: &str = "as downloader set";
