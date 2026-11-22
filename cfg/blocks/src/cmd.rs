use std::{
    io::{self, Write},
    process::Stdio,
};

use anyhow::{Result, bail};
use tap::Tap as _;
use tokio::{io::AsyncWriteExt as _, process};

use crate::{Elevate, ElevateTo};

#[derive(Debug)]
pub struct ExitStatus(usize);

impl ExitStatus {
    pub fn code(&self) -> usize {
        self.0
    }

    pub fn success(&self) -> bool {
        self.0 == 0
    }

    fn failed(&self) -> bool {
        !self.success()
    }
}

#[derive(Debug)]
pub struct ProcessForegroundStatusUnknown {
    status: ExitStatus,
}

impl ProcessForegroundStatusUnknown {
    pub fn check(self) -> Result<ProcessForeground> {
        if self.status.failed() {
            bail!("process failed {self:?}")
        }
        Ok(self.unchecked())
    }

    pub fn unchecked(self) -> ProcessForeground {
        ProcessForeground {
            status: self.status,
        }
    }
}

#[derive(Debug)]
pub struct ProcessForeground {
    pub status: ExitStatus,
}

#[derive(Debug)]
pub struct ProcessRawStatusUnknown {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    status: ExitStatus,
}

#[derive(Debug)]
pub struct ProcessRaw {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: ExitStatus,
}

impl ProcessRawStatusUnknown {
    pub fn check(self) -> Result<ProcessRaw> {
        if self.status.failed() {
            bail!("process failed {self:?}")
        }
        Ok(self.unchecked())
    }

    pub fn unchecked(self) -> ProcessRaw {
        ProcessRaw {
            stdout: self.stdout,
            stderr: self.stderr,
            status: self.status,
        }
    }
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

pub enum Stdin<'a> {
    Closed,
    Write(&'a [u8]),
    Open,
}

impl Process {
    pub async fn run_bin(
        cmd: &str,
        args: &[&str],
        elevate: &Elevate,
        stdin: Stdin<'_>,
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
            .stdin(match stdin {
                Stdin::Closed => Stdio::null(),
                Stdin::Write(_items) => Stdio::piped(),
                Stdin::Open => Stdio::inherit(),
            })
            .spawn()?;

        if let Stdin::Write(data) = stdin {
            tracing::debug!("writing {} bytes to stdin", data.len());
            let mut stdin = cmd.stdin.take().expect("stdin is open");
            stdin.write_all(data).await?;
        }

        let output = cmd.wait_with_output().await?;

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

    pub fn run_bin_sync(
        cmd: &str,
        args: &[&str],
        elevate: &Elevate,
        stdin: &Stdin<'_>,
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

        let mut cmd = std::process::Command::new(cmd)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(match *stdin {
                Stdin::Closed => Stdio::null(),
                Stdin::Write(_items) => Stdio::piped(),
                Stdin::Open => Stdio::inherit(),
            })
            .spawn()?;

        if let Stdin::Write(data) = *stdin {
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

    pub async fn run(
        cmd: &str,
        args: &[&str],
        elevate: &Elevate,
        stdin: Stdin<'_>,
    ) -> Result<ProcessStatusUnknown> {
        let ProcessRawStatusUnknown {
            stdout,
            stderr,
            status,
        } = Self::run_bin(cmd, args, elevate, stdin).await?;

        Ok(ProcessStatusUnknown {
            stdout: stdout.try_into()?,
            stderr: stderr.try_into()?,
            status,
        })
    }

    pub fn run_sync(
        cmd: &str,
        args: &[&str],
        elevate: &Elevate,
        stdin: &Stdin<'_>,
    ) -> Result<ProcessStatusUnknown> {
        let ProcessRawStatusUnknown {
            stdout,
            stderr,
            status,
        } = Self::run_bin_sync(cmd, args, elevate, stdin)?;

        Ok(ProcessStatusUnknown {
            stdout: stdout.try_into()?,
            stderr: stderr.try_into()?,
            status,
        })
    }

    pub fn run_in_foreground(
        cmd: &str,
        args: &[&str],
        elevate: &Elevate,
        stdin: &Stdin<'_>,
    ) -> Result<ProcessForegroundStatusUnknown> {
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

        let mut cmd = std::process::Command::new(cmd)
            .args(args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .stdin(match *stdin {
                Stdin::Closed => Stdio::null(),
                Stdin::Write(_items) => Stdio::piped(),
                Stdin::Open => Stdio::inherit(),
            })
            .spawn()?;

        if let Stdin::Write(data) = *stdin {
            tracing::debug!("writing {} bytes to stdin", data.len());
            let mut stdin = cmd.stdin.take().expect("stdin is open");
            stdin.write_all(data)?;
        }

        let output = cmd.wait()?;

        Ok(ProcessForegroundStatusUnknown {
            status: ExitStatus(
                output
                    .code()
                    .expect("process to have exit code on stop")
                    .try_into()
                    .expect("exit code to be non-negative"),
            ),
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
