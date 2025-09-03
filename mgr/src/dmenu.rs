use std::{fmt::Write as _, num};

use thiserror::Error;
use tracing::{Level, event};

use super::cmd;

#[derive(Debug, Error)]
pub enum Error {
    #[error("rofi did not return an integer: {error}")]
    RofiNonIntOutput { error: num::ParseIntError },
    #[error("rofi returned an invalid indexx: {index}")]
    RofiInvalidIndex { index: usize },
    #[error(transparent)]
    Cmd(#[from] cmd::Error),
}

pub(crate) fn get_choice(actions: &[&'static str]) -> Result<Option<&'static str>, Error> {
    const ROFI: &str = "rofi";

    event!(Level::DEBUG, "starting rofi");

    let process = cmd::command_output_with_stdin_write(
        ROFI,
        &[
            "-dmenu",
            "-p",
            "action",
            "-l",
            &actions.len().to_string(),
            "-no-custom",
            "-sync",
            "-format",
            "i",
        ],
        actions
            .iter()
            .enumerate()
            .fold(String::new(), |mut output, (i, action)| {
                writeln!(
                    output,
                    "({i}) {action}",
                    i = i.checked_add(1).expect("too many action")
                )
                .expect("writing to string cannot fail");
                output
            })
            .as_bytes(),
    )?;

    if process.exit_code == 1 {
        Ok(None)
    } else {
        let choice = process
            .stdout
            .trim()
            .parse::<usize>()
            .map_err(|error| Error::RofiNonIntOutput { error })?;

        Ok(Some(
            actions
                .get(choice)
                .ok_or(Error::RofiInvalidIndex { index: choice })?,
        ))
    }
}
