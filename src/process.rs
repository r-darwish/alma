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
        let exit_status = self.spawn().context(context)?.wait().context(context)?;

        if !exit_status.success() {
            return Err(ProcessError::BadExitCode(exit_status)
                .context(context)
                .into());
        }

        Ok(())
    }

    fn run_text_output(&mut self, context: ErrorKind) -> Result<String, Error> {
        let output = self.output().context(context)?;

        if !output.status.success() {
            let error = str::from_utf8(&output.stderr).unwrap_or("[INVALID UTF8]");
            error!("{}", error);
            return Err(ProcessError::BadExitCode(output.status)
                .context(context)
                .into());
        }

        Ok(String::from(
            str::from_utf8(&output.stdout)
                .map_err(|_| ProcessError::InvalidUtf8)
                .context(context)?,
        ))
    }
}
