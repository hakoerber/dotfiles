use thiserror::Error;

use super::cmd;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Cmd(#[from] cmd::Error),
    #[error("dunstctl is-paused returned unknown output: {output}")]
    DunstctlIsPausedUnknownOutput { output: String },
}

#[derive(Clone, Copy)]
pub(crate) enum Status {
    Paused,
    Unpaused,
}

pub(crate) fn set_status(status: Status) -> Result<(), Error> {
    Ok(cmd::command(
        "dunstctl",
        &[
            "set-paused",
            match status {
                Status::Paused => "true",
                Status::Unpaused => "false",
            },
        ],
    )?)
}

pub(crate) fn is_paused() -> Result<bool, Error> {
    let output = cmd::command_output("dunstctl", &["is-paused"])?;

    match output.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(Error::DunstctlIsPausedUnknownOutput { output }),
    }
}
