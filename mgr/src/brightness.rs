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
    Inc,
    Dec,
}

impl WireCommand for Action {
    fn parse_wire(mut input: impl Iterator<Item = u8>) -> Result<Self, server::ParseError> {
        match input.next().ok_or(server::ParseError::Eof)? {
            0x01 => Ok(Self::Inc),
            0x02 => Ok(Self::Dec),
            byte => Err(server::ParseError::Unknown(byte)),
        }
    }

    fn to_wire(&self) -> Vec<u8> {
        match *self {
            Self::Inc => vec![0x01],
            Self::Dec => vec![0x02],
        }
    }
}

impl CliCommand for Action {
    type ExecErr = Error;

    fn parse_str(input: &str, rest: impl Iterator<Item = String>) -> Result<Self, cli::ParseError>
    where
        Self: Sized,
    {
        let result = match input {
            "inc" => Self::Inc,
            "dec" => Self::Dec,
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
            Self::Inc => cmd::command("brightnessctl", &["set", "8%+"])?,
            Self::Dec => cmd::command("brightnessctl", &["set", "8%-"])?,
        }
        Ok(None)
    }
}
