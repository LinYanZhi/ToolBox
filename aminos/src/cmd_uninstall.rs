use crate::opts::UninstallOpts;
use color;

pub fn run_uninstall(opts: UninstallOpts) -> anyhow::Result<()> {
    for name in &opts.names {
        let n = name.to_lowercase();
        if let Err(e) = crate::installer::uninstall_software(&n, opts.force) {
            eprintln!("  {} {}: {}", color::yellow("跳过"), name, e);
        }
    }
    Ok(())
}
