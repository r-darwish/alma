use anyhow::{Error, Result};
use log::error;
use std::process::Command;
use std::str;

pub trait CommandExt {
    fn run(&mut self, context: Error) -> Result<()>;
    fn run_text_output(&mut self, context: Error) -> Result<String, Error>;
}

impl CommandExt for Command {
    fn run(&mut self, context: Error) -> Result<(), Error> {
        let exit_status = self
            .spawn()
            .with_context(|_| context.clone())?
            .wait()
            .with_context(|_| context.clone())?;

        if !exit_status.success() {
            Err(context)?;
        }

        Ok(())
    }

    fn run_text_output(&mut self, context: Error) -> Result<String, Error> {
        let output = self.output().with_context(|_| context.clone())?;

        if !output.status.success() {
            let error = str::from_utf8(&output.stderr).unwrap_or("[INVALID UTF8]");
            error!("{}", error);
            return Err(context);
        }

        Ok(String::from(str::from_utf8(&output.stdout)?))
    }
}
