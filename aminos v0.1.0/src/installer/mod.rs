mod detect;
mod download;
mod executor;
mod helpers;
mod install_flow;
mod uninstall_flow;
mod windows;

pub use executor::extract_zip_to;
pub use install_flow::{install_software_by_def, install_tool};
pub use uninstall_flow::{uninstall_software, uninstall_tool};
