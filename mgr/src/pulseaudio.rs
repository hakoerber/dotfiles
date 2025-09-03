use thiserror::Error;

use super::{
    Exec,
    cli::{self, CliCommand},
    cmd,
    wire::{WireCommand, server},
};

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Cli(#[from] cli::ParseError),
    #[error(transparent)]
    Cmd(#[from] cmd::Error),
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    InputToggle,
    OutputToggle,
    OutputInc,
    OutputDec,
}

impl WireCommand for Action {
    fn parse_wire(mut input: impl Iterator<Item = u8>) -> Result<Self, server::ParseError> {
        match input.next().ok_or(server::ParseError::Eof)? {
            0x01 => Ok(Self::InputToggle),
            0x02 => Ok(Self::OutputToggle),
            0x03 => Ok(Self::OutputInc),
            0x04 => Ok(Self::OutputDec),
            byte => Err(server::ParseError::Unknown(byte)),
        }
    }

    fn to_wire(&self) -> Vec<u8> {
        match *self {
            Self::InputToggle => vec![0x01],
            Self::OutputToggle => vec![0x02],
            Self::OutputInc => vec![0x03],
            Self::OutputDec => vec![0x04],
        }
    }
}

impl CliCommand for Action {
    type ExecErr = Error;

    fn parse_str(
        input: &str,
        mut rest: impl Iterator<Item = String>,
    ) -> Result<Self, cli::ParseError>
    where
        Self: Sized,
    {
        let result = match input {
            "input" => match rest.next().ok_or(cli::ParseError::MissingAction)?.as_str() {
                "toggle" => Self::InputToggle,
                s => {
                    return Err(cli::ParseError::UnknownAction {
                        action: s.to_owned(),
                    });
                }
            },
            "output" => match rest.next().ok_or(cli::ParseError::MissingAction)?.as_str() {
                "toggle" => Self::OutputToggle,
                "inc" => Self::OutputInc,
                "dec" => Self::OutputDec,
                s => {
                    return Err(cli::ParseError::UnknownAction {
                        action: s.to_owned(),
                    });
                }
            },
            s => {
                return Err(cli::ParseError::UnknownAction {
                    action: s.to_owned(),
                });
            }
        };

        let rest = rest.collect::<Vec<String>>();
        if rest.is_empty() {
            Ok(result)
        } else {
            Err(cli::ParseError::UnexpectedInput { rest })
        }
    }
}

impl Exec for Action {
    type ExecErr = Error;

    fn execute(&self) -> Result<Option<String>, Self::ExecErr> {
        match *self {
            Self::InputToggle => {
                cmd::command("pactl", &["set-source-mute", "@DEFAULT_SOURCE@", "toggle"])?;
            }
            Self::OutputToggle => {
                cmd::command("pactl", &["set-sink-mute", "@DEFAULT_SINK@", "toggle"])?;
            }
            Self::OutputInc => {
                cmd::command("pactl", &["set-sink-volume", "@DEFAULT_SINK@", "+5%"])?;
            }
            Self::OutputDec => {
                cmd::command("pactl", &["set-sink-volume", "@DEFAULT_SINK@", "-5%"])?;
            }
        }
        Ok(None)
    }
}
