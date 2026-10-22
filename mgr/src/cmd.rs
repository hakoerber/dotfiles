use std::{
    io::{self, Write as _},
    process, str,
};

use thiserror::Error;
use tracing::{Level, event};

#[derive(Error, Debug)]
pub enum Error {
    #[error("command \"{program}\" failed: {error}")]
    CommandInvocation { program: String, error: io::Error },
    #[error("command \"{program}\" was terminated by signal")]
    CommandTerminatedBySignal { program: String },
    #[error(
        "command \"{program}\" failed [{code}]: {stderr}",
        code = {
            match *.code {
                Some(code) => code.to_string(),
                _ => "unknown exit code".to_owned(),
            }
        },
        stderr = if .stderr.is_empty() {
            "[stderr empty]"
        } else {
            .stderr
    })]
    CommandFailed {
        program: String,
        code: Option<i32>,
        stderr: String,
    },
    #[error("command \"{program}\" produced non-utf8 output: {error}")]
    CommandOutputNonUtf8 {
        program: String,
        error: str::Utf8Error,
    },
    #[error("failed writing to stdin of command \"{program}\": {error}")]
    StdinWriteFailed { program: String, error: io::Error },
    #[error("killing command \"{program}\" failed: {error}")]
    KillFailed { program: String, error: io::Error },
}

pub(crate) struct Command<'cmd> {
    program: &'cmd str,
    args: &'cmd [&'cmd str],
    process: process::Child,
}

impl<'cmd> Command<'cmd> {
    pub(crate) fn start(program: &'cmd str, args: &'cmd [&'cmd str]) -> Result<Self, Error> {
        event!(Level::DEBUG, "starting {program} {args:?}");

        let process = process::Command::new(program)
            .args(args)
            .stdin(process::Stdio::piped())
            .stdout(process::Stdio::piped())
            .stderr(process::Stdio::piped())
            .spawn()
            .map_err(|error| Error::CommandInvocation {
                program: program.to_owned(),
                error,
            })?;

        Ok(Command {
            program,
            args,
            process,
        })
    }

    pub(crate) fn write(self, input: &[u8]) -> Result<Self, Error> {
        let mut stdin = self
            .process
            .stdin
            .as_ref()
            .expect("stdin handle must be present");

        stdin
            .write_all(input)
            .map_err(|error| Error::StdinWriteFailed {
                program: self.program.to_owned(),
                error,
            })?;

        Ok(self)
    }

    pub(crate) fn run(
        command: &'cmd str,
        args: &'cmd [&'cmd str],
    ) -> Result<FinishedProcess<'cmd>, Error> {
        event!(Level::DEBUG, "running {command} {args:?}");
        Self::start(command, args)?.wait()
    }

    pub fn wait(self) -> Result<FinishedProcess<'cmd>, Error> {
        let output = self
            .process
            .wait_with_output()
            .map_err(|error| Error::CommandInvocation {
                program: self.program.to_owned(),
                error,
            })?;

        Ok(FinishedProcess {
            exit_code: output
                .status
                .code()
                .ok_or_else(|| Error::CommandTerminatedBySignal {
                    program: self.program.to_owned(),
                })?,
            stdout: str::from_utf8(&output.stdout)
                .map_err(|error| Error::CommandOutputNonUtf8 {
                    program: self.program.to_owned(),
                    error,
                })?
                .to_owned(),
            stderr: str::from_utf8(&output.stderr)
                .map_err(|error| Error::CommandOutputNonUtf8 {
                    program: self.program.to_owned(),
                    error,
                })?
                .to_owned(),
            command: self.program,
            args: self.args,
        })
    }

    pub(crate) fn with<F: Fn() -> Result<(), E>, E: From<Error>>(
        self,
        f: F,
    ) -> Result<FinishedProcess<'cmd>, E> {
        f()?;
        event!(
            Level::DEBUG,
            "waiting for process {} to finish",
            self.program
        );
        let result = self.wait()?;
        Ok(result)
    }
}

#[derive(Debug)]
pub(crate) struct FinishedProcess<'cmd> {
    pub command: &'cmd str,
    pub args: &'cmd [&'cmd str],
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl FinishedProcess<'_> {
    pub(crate) fn check(self) -> Result<Self, Error> {
        if self.exit_code != 0_i32 {
            event!(
                Level::DEBUG,
                "{command} {args:?} failed",
                command = self.command,
                args = self.args
            );
            return Err(Error::CommandFailed {
                program: self.command.to_owned(),
                code: Some(self.exit_code),
                stderr: self.stderr,
            });
        }

        Ok(self)
    }
}
