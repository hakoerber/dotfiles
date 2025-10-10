use super::{
    Exec,
    cli::{self, CliCommand},
    cmd,
    wire::{WireCommand, server},
};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Cmd(#[from] cmd::Error),
    #[error(transparent)]
    Cli(#[from] cli::ParseError),
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Sleep(std::time::Duration),
}

impl WireCommand for Action {
    fn parse_wire(mut input: impl Iterator<Item = u8>) -> Result<Self, server::ParseError> {
        match input.next().ok_or(server::ParseError::Eof)? {
            0x01 => {
                const BYTES: usize = (u64::BITS / 8) as usize;
                let input = input.take(BYTES).collect::<Vec<u8>>();
                let input: [u8; BYTES] =
                    input
                        .try_into()
                        .map_err(|vec: Vec<u8>| server::ParseError::MissingBytes {
                            expected: BYTES,
                            received: vec.len(),
                        })?;

                let secs = u64::from_le_bytes(input);
                let duration = std::time::Duration::from_secs(secs);
                Ok(Self::Sleep(duration))
            }
            byte => Err(server::ParseError::Unknown(byte)),
        }
    }

    fn to_wire(&self) -> Vec<u8> {
        match *self {
            Self::Sleep(duration) => {
                let mut v = vec![0x01];
                v.extend(duration.as_secs().to_le_bytes());
                v
            }
        }
    }
}

impl Exec for Action {
    type ExecErr = Error;

    fn execute(&self) -> Result<Option<String>, Self::ExecErr> {
        match *self {
            Self::Sleep(duration) => {
                std::thread::sleep(duration);
                Ok(None)
            }
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
            "sleep" => {
                let input = rest.next().ok_or(cli::ParseError::MissingArgument)?;
                let seconds =
                    input
                        .parse::<u64>()
                        .map_err(|err| cli::ParseError::ArgumentParse {
                            message: err.to_string(),
                        })?;
                Self::Sleep(std::time::Duration::from_secs(seconds))
            }
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
