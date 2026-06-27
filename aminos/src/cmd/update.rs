/// as update — 更新自身（通过 as tool upgrade）
///
/// 暂时只输出提示，后续可以接入自更新逻辑。
pub fn run() -> anyhow::Result<()> {
    println!("  更新 as 自身...");
    println!("  {} 暂未实现", color::yellow("提示:"));
    println!("  请重新编译安装最新版本");
    Ok(())
}
