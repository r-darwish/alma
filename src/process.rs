use anyhow::anyhow;
use log::error;
use std::process::Command;
use std::str;

pub trait CommandExt {
    fn run(&mut self) -> anyhow::Result<()>;
    fn run_text_output(&mut self) -> anyhow::Result<String>;
}

impl CommandExt for Command {
    fn run(&mut self) -> anyhow::Result<()> {
        let exit_status = self.spawn()?.wait()?;

        if !exit_status.success() {
            Err(anyhow!("Bad exit code: {}", exit_status))?;
        }

        Ok(())
    }

    fn run_text_output(&mut self) -> anyhow::Result<String> {
        let output = self.output()?;

        if !output.status.success() {
            let error = str::from_utf8(&output.stderr).unwrap_or("[INVALID UTF8]");
            error!("{}", error);
            Err(anyhow!("Bad exit code: {}", output.status))?;
        }

        Ok(String::from(str::from_utf8(&output.stdout).map_err(
            |_| anyhow!("Process output is not valid UTF-8"),
        )?))
    }
}
