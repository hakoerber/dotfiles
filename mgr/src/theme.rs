use thiserror::Error;

use super::{
    Exec,
    cli::{self, CliCommand},
    cmd, systemd,
    wire::{WireCommand, server},
};

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Cli(#[from] cli::ParseError),
    #[error(transparent)]
    Cmd(#[from] cmd::Error),
    #[error(transparent)]
    Systemd(#[from] systemd::Error),
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Dark = 0x01,
    Light = 0x02,
    Status = 0x03,
}

impl WireCommand for Action {
    fn parse_wire(mut input: impl Iterator<Item = u8>) -> Result<Self, server::ParseError> {
        match input.next().ok_or(server::ParseError::Eof)? {
            0x01 => Ok(Self::Dark),
            0x02 => Ok(Self::Light),
            0x03 => Ok(Self::Status),
            byte => Err(server::ParseError::Unknown(byte)),
        }
    }

    fn to_wire(&self) -> Vec<u8> {
        match *self {
            Self::Dark => vec![0x01],
            Self::Light => vec![0x02],
            Self::Status => vec![0x03],
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
            "dark" => Self::Dark,
            "light" => Self::Light,
            "status" => Self::Status,
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
            Self::Dark => {
                cmd::command(
                    "systemctl",
                    &["--user", "--no-block", "start", "color-theme-dark.service"],
                )?;
                Ok(None)
            }
            Self::Light => {
                cmd::command(
                    "systemctl",
                    &["--user", "--no-block", "start", "color-theme-light.service"],
                )?;
                Ok(None)
            }
            Self::Status => Ok(
                if systemd::user::unit_status("color-theme-light.service")?.is_active() {
                    Some("light".to_owned())
                } else {
                    Some("dark".to_owned())
                },
            ),
        }
    }
}
