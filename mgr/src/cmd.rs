use std::{io, panic, process, str, thread};

use thiserror::Error;
use tracing::{Level, event};

#[derive(Error, Debug)]
pub enum Error {
    #[error("command \"{command}\" failed: {error}")]
    CommandInvocation {
        command: &'static str,
        error: io::Error,
    },
    #[error("command \"{command}\" was terminated by signal")]
    CommandTerminatedBySignal { command: &'static str },
    #[error(
        "command \"{command}\" failed [{code}]: {stderr}",
        code = match *.code {
            Some(code) => &.code.to_string(),
            _ => "unknown exit code",
        },
        stderr = if .stderr.is_empty() {
            "[stderr empty]"
        } else {
            .stderr
    })]
    CommandFailed {
        command: &'static str,
        code: Option<i32>,
        stderr: String,
    },
    #[error("{command} produced non-utf8 output: {error}")]
    CommandOutputNonUtf8 {
        command: &'static str,
        error: str::Utf8Error,
    },
    #[error("failed writing to stdin of command \"{command}\": {error}")]
    StdinWriteFailed {
        command: &'static str,
        error: io::Error,
    },
}

pub(crate) fn command(command: &'static str, args: &[&str]) -> Result<(), Error> {
    let _: FinishedProcess = run_command_checked(command, args)?;
    Ok(())
}

pub(crate) fn run_command(command: &'static str, args: &[&str]) -> Result<FinishedProcess, Error> {
    event!(Level::DEBUG, "running {command} {args:?}");
    let proc = process::Command::new(command)
        .args(args)
        .output()
        .map_err(|error| Error::CommandInvocation { command, error })?;

    Ok(FinishedProcess {
        exit_code: proc
            .status
            .code()
            .ok_or(Error::CommandTerminatedBySignal { command })?,
        stdout: str::from_utf8(&proc.stdout)
            .map_err(|error| Error::CommandOutputNonUtf8 { command, error })?
            .to_owned(),
        stderr: str::from_utf8(&proc.stderr)
            .map_err(|error| Error::CommandOutputNonUtf8 { command, error })?
            .to_owned(),
    })
}

pub(crate) fn run_command_checked(
    command: &'static str,
    args: &[&str],
) -> Result<FinishedProcess, Error> {
    let output = run_command(command, args)?;

    if output.exit_code != 0_i32 {
        event!(Level::DEBUG, "{command} {args:?} failed");
        return Err(Error::CommandFailed {
            command,
            code: Some(output.exit_code),
            stderr: output.stderr,
        });
    }

    Ok(output)
}

pub(crate) fn command_output(command: &'static str, args: &[&str]) -> Result<String, Error> {
    let output = run_command_checked(command, args)?;
    Ok(output.stdout)
}

#[derive(Debug)]
pub(crate) struct FinishedProcess {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub(crate) fn command_output_with_stdin_write(
    command: &'static str,
    args: &[&str],
    input: &[u8],
) -> Result<FinishedProcess, Error> {
    use io::Write as _;

    let process = process::Command::new(command)
        .args(args)
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::null())
        .spawn()
        .map_err(|error| Error::CommandInvocation { command, error })?;

    let mut stdin = process
        .stdin
        .as_ref()
        .expect("stdin handle must be present");

    stdin
        .write_all(input)
        .map_err(|error| Error::StdinWriteFailed { command, error })?;

    let output = process
        .wait_with_output()
        .map_err(|error| Error::CommandInvocation { command, error })?;

    let exit_code = output
        .status
        .code()
        .ok_or(Error::CommandTerminatedBySignal { command })?;

    let stdout = str::from_utf8(&output.stdout)
        .map_err(|error| Error::CommandOutputNonUtf8 { command, error })?
        .to_owned();

    let stderr = str::from_utf8(&output.stderr)
        .map_err(|error| Error::CommandOutputNonUtf8 { command, error })?
        .to_owned();

    Ok(FinishedProcess {
        exit_code,
        stdout,
        stderr,
    })
}

pub(crate) struct RunningProcess {
    command: &'static str,
    join_handle: thread::JoinHandle<Result<FinishedProcess, Error>>,
}

impl RunningProcess {
    pub fn with<F: Fn() -> Result<(), E>, E: From<Error>>(
        self,
        f: F,
    ) -> Result<FinishedProcess, E> {
        f()?;
        event!(
            Level::DEBUG,
            "waiting for process {} to finish",
            self.command
        );
        let ret = match self.join_handle.join() {
            Ok(ret) => ret?,
            Err(e) => panic::resume_unwind(e),
        };
        event!(Level::DEBUG, "process {} finished", self.command);
        Ok(ret)
    }
}

pub(crate) fn start_command(
    command: &'static str,
    args: &'static [&'static str],
) -> RunningProcess {
    event!(Level::DEBUG, "starting {command} {args:?}");
    let join_handle = thread::spawn(move || run_command_checked(command, args));

    RunningProcess {
        command,
        join_handle,
    }
}
