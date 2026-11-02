use std::{
    io::{self, Write},
    process::{self, Stdio},
};

use anyhow::{Result, bail};
use tap::Tap as _;

use super::{Elevate, ElevateTo};

#[derive(Debug)]
pub struct ExitStatus(usize);

impl ExitStatus {
    pub(crate) fn code(&self) -> usize {
        self.0
    }

    pub(crate) fn success(&self) -> bool {
        self.0 == 0
    }

    fn failed(&self) -> bool {
        !self.success()
    }
}

#[derive(Debug)]
pub(crate) struct ProcessRawStatusUnknown {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    status: ExitStatus,
}

#[derive(Debug)]
pub(crate) struct ProcessRaw {
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) status: ExitStatus,
}

impl ProcessRawStatusUnknown {
    pub(crate) fn check(self) -> Result<ProcessRaw> {
        if self.status.failed() {
            bail!("process failed {self:?}")
        }
        Ok(self.unchecked())
    }

    pub(crate) fn unchecked(self) -> ProcessRaw {
        ProcessRaw {
            stdout: self.stdout,
            stderr: self.stderr,
            status: self.status,
        }
    }
}

pub(crate) fn cmd_bin(
    cmd: &str,
    args: &[&str],
    elevate: &Elevate,
    stdin: Option<&[u8]>,
) -> io::Result<ProcessRawStatusUnknown> {
    let (cmd, args) = match *elevate {
        Elevate::Sudo(ref elevate_to) => match *elevate_to {
            ElevateTo::Root => ("sudo", vec!["--"]),
            ElevateTo::User { ref user } => ("sudo", vec!["-u", user, "--"]),
        }
        .tap_mut(|v| {
            v.1.push(cmd);
            v.1.extend_from_slice(args);
        }),
        Elevate::No => (cmd, args.to_vec()),
    };

    tracing::debug!("running cmd {cmd} {args:?}");

    let mut cmd = process::Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .spawn()?;

    if let Some(data) = stdin {
        tracing::debug!("writing {} bytes to stdin", data.len());
        let mut stdin = cmd.stdin.take().expect("stdin is open");
        stdin.write_all(data)?;
    }

    let output = cmd.wait_with_output()?;

    tracing::debug!("received {output:?}");

    Ok(ProcessRawStatusUnknown {
        stdout: output.stdout,
        stderr: output.stderr,
        status: ExitStatus(
            output
                .status
                .code()
                .expect("process to have exit code on stop")
                .try_into()
                .expect("exit code to be non-negative"),
        ),
    })
}

#[derive(Debug)]
pub struct Process {
    pub stdout: String,
    pub stderr: String,
    pub status: ExitStatus,
}

#[derive(Debug)]
#[must_use]
pub struct ProcessStatusUnknown {
    stdout: String,
    stderr: String,
    status: ExitStatus,
}

impl Process {
    pub fn run(
        cmd: &str,
        args: &[&str],
        elevate: &Elevate,
        stdin: Option<&[u8]>,
    ) -> Result<ProcessStatusUnknown> {
        let ProcessRawStatusUnknown {
            stdout,
            stderr,
            status,
        } = cmd_bin(cmd, args, elevate, stdin)?;

        Ok(ProcessStatusUnknown {
            stdout: stdout.try_into()?,
            stderr: stderr.try_into()?,
            status,
        })
    }
}

impl ProcessStatusUnknown {
    pub fn check(self) -> Result<Process> {
        if self.status.failed() {
            bail!("process failed {self:?}")
        }
        Ok(self.unchecked())
    }

    pub fn unchecked(self) -> Process {
        Process {
            stdout: self.stdout,
            stderr: self.stderr,
            status: self.status,
        }
    }
}
