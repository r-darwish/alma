use super::error::*;
use failure::{Fail, ResultExt};
use log::error;
use std::process::{Command, ExitStatus};
use std::str;

#[derive(Debug, Fail)]
enum ProcessError {
    #[fail(display = "{}", _0)]
    BadExitCode(ExitStatus),

    #[fail(display = "Process output isn't valid UTF-8")]
    InvalidUtf8,
}

pub trait CommandExt {
    fn run(&mut self, context: ErrorKind) -> Result<(), Error>;
    fn run_text_output(&mut self, context: ErrorKind) -> Result<String, Error>;
}

impl CommandExt for Command {
    fn run(&mut self, context: ErrorKind) -> Result<(), Error> {
        let exit_status = self
            .spawn()
            .with_context(|_| context.clone())?
            .wait()
            .with_context(|_| context.clone())?;

        if !exit_status.success() {
            Err(ProcessError::BadExitCode(exit_status)).with_context(|_| context.clone())?;
        }

        Ok(())
    }

    fn run_text_output(&mut self, context: ErrorKind) -> Result<String, Error> {
        let output = self.output().with_context(|_| context.clone())?;

        if !output.status.success() {
            let error = str::from_utf8(&output.stderr).unwrap_or("[INVALID UTF8]");
            error!("{}", error);
            Err(ProcessError::BadExitCode(output.status)).with_context(|_| context.clone())?;
        }

        Ok(String::from(
            str::from_utf8(&output.stdout)
                .map_err(|_| ProcessError::InvalidUtf8)
                .with_context(|_| context.clone())?,
        ))
    }
}
