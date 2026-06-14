pub fn run_downloader(action: crate::DownloaderCmd) -> anyhow::Result<()> {
    match action {
        crate::DownloaderCmd::List => {
            crate::downloader::run_downloader_list()?;
        }
        crate::DownloaderCmd::Set { name, state } => {
            let enable = match state.as_str() {
                "on" => true,
                "off" => false,
                _ => anyhow::bail!("无效状态: {}（使用 on/off）", state),
            };
            crate::downloader::run_downloader_set(&name, enable)?;
        }
        crate::DownloaderCmd::Config { open } => {
            crate::downloader::run_downloader_config(open)?;
        }
    }
    Ok(())
}
