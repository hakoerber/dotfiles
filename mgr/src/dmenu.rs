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

    let action_len = actions.len().to_string();

    let args = &[
        "-dmenu",
        "-p",
        "action",
        "-l",
        action_len.as_str(),
        "-no-custom",
        "-sync",
        "-format",
        "i",
    ];

    let process = cmd::Command::start(ROFI, args)?
        .write(
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
        )?
        .wait()?;

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
