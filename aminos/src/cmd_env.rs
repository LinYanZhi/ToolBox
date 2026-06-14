use crate::{cmd_cache, cmd_downloader, cmd_source, EnvCmd};

pub fn run_env(action: EnvCmd) -> anyhow::Result<()> {
    match action {
        EnvCmd::Cache { clear, open } => {
            cmd_cache::run_cache(clear, open)?;
        }
        EnvCmd::Source { action } => {
            cmd_source::run_source(action)?;
        }
        EnvCmd::Downloader { action } => {
            cmd_downloader::run_downloader(action)?;
        }
    }
    Ok(())
}
