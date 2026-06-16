use crate::{paths, software};

pub fn run_source(update: bool, clear: bool, open: bool, speedtest: bool, names: Vec<String>, software_flag: bool) -> anyhow::Result<()> {
    if open {
        let dir = paths::source_dir();
        if dir.exists() {
            let _ = std::process::Command::new("explorer").arg(&dir).spawn();
            println!("已在资源管理器中打开: {}", dir.display());
        } else {
            println!("源目录不存在: {}", dir.display());
        }
        return Ok(());
    }

    if clear {
        return clear_sources();
    }

    if update {
        return software::update_sources();
    }

    if speedtest {
        return crate::speedtest::speedtest(&names, software_flag);
    }

    Ok(())
}

fn clear_sources() -> anyhow::Result<()> {
    let dir = paths::source_dir();
    if !dir.exists() {
        println!("源目录不存在: {}", dir.display());
        return Ok(());
    }

    let count = std::fs::read_dir(&dir)?.count();
    std::fs::remove_dir_all(&dir)?;
    std::fs::create_dir_all(&dir)?;
    println!("已清空源目录（共 {} 个文件）: {}", count, dir.display());
    Ok(())
}
