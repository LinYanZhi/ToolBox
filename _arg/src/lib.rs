//! ToolBox 自研 CLI 框架
//!
//! ## 设计理念
//!
//! - 零额外依赖（仅依赖 workspace 内的 `color` 库）
//! - 纯中文帮助输出，动态生成，格式统一
//! - Builder API，直观易维护
//!
//! ## 快速上手
//!
//! ```ignore
//! use arg::*;
//!
//! init(); // 启用 ANSI
//!
//! let cmd = Cmd::new("mytool")
//!     .about("极简演示工具")
//!     .arg(flag("help", 'h', "显示帮助"))
//!     .arg(arg("output", 'o', "输出路径").default("./out"));
//!
//! let args = parse(&cmd, &std::env::args().collect::<Vec<_>>())?;
//! if args.flag("help") {
//!     print_help(&cmd, &std::env::current_exe().unwrap().display().to_string());
//!     return;
//! }
//! ```

mod arg;
mod cmd;
mod help;
mod parse;
mod theme;
#[cfg(test)]
mod tests;

pub use arg::ArgDef;
pub use cmd::{Cmd, ParsedArgs};
pub use help::{print_error, print_help, print_version};
pub use parse::parse;
pub use theme::Theme;

// ── 便捷构造器 ──

/// 创建 Flag（布尔开关）
pub fn flag(long: &str, short: char, desc: &str) -> ArgDef {
    ArgDef::flag(long, Some(short), desc)
}

/// 创建只有长名的 Flag
pub fn flag_long(long: &str, desc: &str) -> ArgDef {
    ArgDef::flag(long, None, desc)
}

/// 创建带值参数
pub fn arg(long: &str, short: char, desc: &str) -> ArgDef {
    ArgDef::value(long, Some(short), desc)
}

/// 创建只有长名的带值参数
pub fn arg_long(long: &str, desc: &str) -> ArgDef {
    ArgDef::value(long, None, desc)
}

/// 创建位置参数
pub fn pos(long: &str, desc: &str) -> ArgDef {
    ArgDef::value(long, None, desc).positional()
}

/// 启用 ANSI 色彩输出
pub fn init() {
    color::enable_ansi();
}
