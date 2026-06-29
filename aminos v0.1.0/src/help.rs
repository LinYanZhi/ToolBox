use color;

/// 包装函数：执行操作并统一处理错误
pub fn run<F: FnOnce() -> anyhow::Result<()>>(f: F) -> i32 {
    match f() {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("  {}", color::bold_red(format!("错误: {}", e)));
            1
        }
    }
}
