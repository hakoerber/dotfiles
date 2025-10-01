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
    Cmd(#[from] cmd::Error),
    #[error(transparent)]
    Cli(#[from] cli::ParseError),
    #[error(transparent)]
    Systemd(#[from] systemd::Error),
    #[error("spotify does not seem to be running")]
    NotFound,
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Start,
    Stop,
    Status,
    Play,
    Pause,
    Toggle,
    Previous,
    Next,
}

impl WireCommand for Action {
    fn parse_wire(mut input: impl Iterator<Item = u8>) -> Result<Self, server::ParseError> {
        match input.next().ok_or(server::ParseError::Eof)? {
            0x01 => Ok(Self::Start),
            0x02 => Ok(Self::Stop),
            0x03 => Ok(Self::Status),
            0x04 => Ok(Self::Play),
            0x05 => Ok(Self::Pause),
            0x06 => Ok(Self::Toggle),
            0x07 => Ok(Self::Previous),
            0x08 => Ok(Self::Next),
            byte => Err(server::ParseError::Unknown(byte)),
        }
    }

    fn to_wire(&self) -> Vec<u8> {
        match *self {
            Self::Start => vec![0x01],
            Self::Stop => vec![0x02],
            Self::Status => vec![0x03],
            Self::Play => vec![0x04],
            Self::Pause => vec![0x05],
            Self::Toggle => vec![0x06],
            Self::Previous => vec![0x07],
            Self::Next => vec![0x08],
        }
    }
}

impl Exec for Action {
    type ExecErr = Error;

    fn execute(&self) -> Result<Option<String>, Self::ExecErr> {
        match *self {
            Self::Start => {
                set(Status::On)?;
                Ok(None)
            }
            Self::Stop => {
                set(Status::Off)?;
                Ok(None)
            }
            Self::Status => Ok(
                if systemd::user::unit_status("spotify.service")?.is_active() {
                    Some("active".to_owned())
                } else {
                    Some("inactive".to_owned())
                },
            ),
            Self::Play => {
                playerctl("play")?;
                Ok(None)
            }
            Self::Pause => {
                playerctl("pause")?;
                Ok(None)
            }
            Self::Toggle => {
                playerctl("play-pause")?;
                Ok(None)
            }
            Self::Previous => {
                playerctl("previous")?;
                Ok(None)
            }
            Self::Next => {
                playerctl("next")?;
                Ok(None)
            }
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
            "start" => Self::Start,
            "stop" => Self::Stop,
            "status" => Self::Status,
            "play" => Self::Play,
            "pause" => Self::Pause,
            "toggle" => Self::Toggle,
            "previous" => Self::Previous,
            "next" => Self::Next,
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

#[derive(Debug, Clone, Copy)]
pub(crate) enum Status {
    On,
    Off,
}

pub(crate) fn set(status: Status) -> Result<(), Error> {
    Ok(cmd::command(
        "systemctl",
        &[
            "--user",
            "--no-block",
            match status {
                Status::On => "start",
                Status::Off => "stop",
            },
            "spotify.service",
        ],
    )?)
}

fn playerctl(cmd: &str) -> Result<(), Error> {
    if cmd::run_command("playerctl", &["-p", "spotify", cmd])?
        .stderr
        .contains("No players found")
    {
        Err(Error::NotFound)
    } else {
        Ok(())
    }
}

pub(crate) fn pause() -> Result<(), Error> {
    playerctl("pause")
}
