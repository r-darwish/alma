mod chroot;
mod mount;
mod qemu;

pub use chroot::chroot;
pub use mount::mount;
pub use qemu::qemu;

use anyhow::anyhow;
use std::path::PathBuf;
use std::process::Command;
use which::which;

#[derive(Debug)]
pub struct Tool {
    exec: PathBuf,
}

impl Tool {
    pub fn find(name: &'static str) -> anyhow::Result<Self> {
        // Note this conversion is only necessary until which releases their new version using
        // thiserror instead of failure - then we can just use .with_context() on the thiserror
        // Error
        // Commit pending release:
        // BLOCKED: https://github.com/harryfei/which-rs/commit/e6e839c4f6cdf8d3e33ec7eafdd50d34472740ea
        let which = match which(name) {
            Ok(x) => Ok(x),
            Err(_) => Err(anyhow!("Could not find tool: {}", name)),
        }?;
        Ok(Self { exec: which })
    }

    pub fn execute(&self) -> Command {
        Command::new(&self.exec)
    }
}
