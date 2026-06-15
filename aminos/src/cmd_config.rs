use crate::{cmd_cache, cmd_downloader, cmd_source, help, ConfigCmd};

pub fn run_config(action: ConfigCmd) -> anyhow::Result<()> {
    match action {
        ConfigCmd::Path { open } => {
            cmd_source::run_dirs(open)?;
        }
        ConfigCmd::Cache { clear, open } => {
            cmd_cache::run_cache(clear, open)?;
        }
        ConfigCmd::Source { action: Some(action) } => {
            cmd_source::run_source(action)?;
        }
        ConfigCmd::Source { action: None } => {
            help::print_source_help();
        }
        ConfigCmd::Speedtest { name, software } => {
            crate::speedtest::speedtest(&name, software)?;
        }
        ConfigCmd::Downloader { action: Some(action) } => {
            cmd_downloader::run_downloader(action)?;
        }
        ConfigCmd::Downloader { action: None } => {
            help::print_downloader_help();
        }
    }
    Ok(())
}
