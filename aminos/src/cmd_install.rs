use crate::opts::InstallOpts;
use color;

pub fn run_install(opts: InstallOpts) -> anyhow::Result<()> {
    for name in &opts.names {
        let n = name.to_lowercase();
        if let Err(e) = crate::installer::install_software(&n, &opts) {
            eprintln!("  {} {}: {}", color::yellow("跳过"), name, e);
        }
    }
    Ok(())
}
