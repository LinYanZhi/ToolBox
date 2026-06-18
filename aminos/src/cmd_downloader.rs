use crate::downloader;

pub fn run_downloader(cmd: &crate::opts::DownloaderCommand) -> anyhow::Result<()> {
    match cmd {
        crate::opts::DownloaderCommand::List { verbose } => {
            downloader::run_downloader_list(*verbose)
        }
        crate::opts::DownloaderCommand::Set { name, state } => {
            let enable = match state.as_str() {
                "on" => true,
                "off" => false,
                _ => anyhow::bail!("无效状态: {}（使用 on/off）", state),
            };
            downloader::run_downloader_set(name, enable)
        }
        crate::opts::DownloaderCommand::Open => {
            downloader::run_downloader_config(true)
        }
    }
}
