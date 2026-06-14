use crate::{cmd_init, cmd_self_update, SelfCmd};

pub fn run_self(action: SelfCmd) -> anyhow::Result<()> {
    match action {
        SelfCmd::Init => {
            cmd_init::run_init()?;
        }
        SelfCmd::Update => {
            cmd_self_update::run_self_update()?;
        }
    }
    Ok(())
}
