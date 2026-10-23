use std::process;

use anyhow::Result;
use tap::Tap as _;

use super::{Elevate, ElevateTo};

#[derive(Debug)]
pub(crate) struct ExitStatus(usize);

impl ExitStatus {
    pub(crate) fn code(&self) -> usize {
        self.0
    }
}

#[derive(Debug)]
pub(crate) struct Process {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) status: ExitStatus,
}

pub(crate) fn cmd(args: &[&str], elevate: &Elevate) -> Result<Process> {
    let command = match *elevate {
        Elevate::Sudo(ref elevate_to) => match *elevate_to {
            ElevateTo::Root => vec!["sudo", "--"].tap_mut(|v| v.extend_from_slice(args)),
            ElevateTo::User { ref user } => {
                vec!["sudo", "-u", user, "--"].tap_mut(|v| v.extend_from_slice(args))
            }
        },
        Elevate::No => args.to_vec(),
    };

    let output = process::Command::new(command[0])
        .args(&command[1..])
        .output()?;

    Ok(Process {
        stdout: output.stdout.try_into()?,
        stderr: output.stderr.try_into()?,
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
