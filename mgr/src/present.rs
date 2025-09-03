use std::{fs, io, path::PathBuf};

use thiserror::Error;

use super::{
    Exec,
    cli::{self, CliCommand},
    cmd, dirs, dunst, redshift, spotify,
    wire::{WireCommand, server},
};

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("unknown action: {action}")]
    UnknownAction { action: String },
    #[error(transparent)]
    Cli(#[from] cli::ParseError),
    #[error(transparent)]
    Dirs(#[from] dirs::Error),
    #[error(transparent)]
    Dunst(#[from] dunst::Error),
    #[error(transparent)]
    Cmd(#[from] cmd::Error),
    #[error(transparent)]
    Redshift(#[from] redshift::Error),
    #[error(transparent)]
    Spotify(#[from] spotify::Error),
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    On,
    Off,
    Toggle,
    Status,
}

impl WireCommand for Action {
    fn parse_wire(mut input: impl Iterator<Item = u8>) -> Result<Self, server::ParseError> {
        match input.next().ok_or(server::ParseError::Eof)? {
            0x01 => Ok(Self::On),
            0x02 => Ok(Self::Off),
            0x03 => Ok(Self::Toggle),
            0x04 => Ok(Self::Status),
            byte => Err(server::ParseError::Unknown(byte)),
        }
    }

    fn to_wire(&self) -> Vec<u8> {
        match *self {
            Self::On => vec![0x01],
            Self::Off => vec![0x02],
            Self::Toggle => vec![0x03],
            Self::Status => vec![0x04],
        }
    }
}

fn status_file() -> Result<PathBuf, Error> {
    Ok(dirs::require_xdg_runtime_dir()?.join("presentation-mode-on"))
}

#[derive(Debug, Clone, Copy)]
enum Status {
    On,
    Off,
}

fn status() -> Result<Status, Error> {
    Ok(if status_file()?.exists() {
        Status::On
    } else {
        Status::Off
    })
}

fn on() -> Result<(), Error> {
    drop(fs::File::create(status_file()?)?);
    dunst::set_status(dunst::Status::Paused)?;
    redshift::set(redshift::Status::Off)?;
    spotify::set(spotify::Status::Off)?;
    Ok(())
}

fn off() -> Result<(), Error> {
    fs::remove_file(status_file()?)?;
    dunst::set_status(dunst::Status::Unpaused)?;
    redshift::set(redshift::Status::On)?;
    spotify::set(spotify::Status::On)?;
    Ok(())
}

fn toggle() -> Result<(), Error> {
    match status()? {
        Status::On => off()?,
        Status::Off => on()?,
    }
    Ok(())
}

impl Exec for Action {
    type ExecErr = Error;

    fn execute(&self) -> Result<Option<String>, Self::ExecErr> {
        match *self {
            Self::On => {
                on()?;
                Ok(None)
            }
            Self::Off => {
                off()?;
                Ok(None)
            }
            Self::Toggle => {
                toggle()?;
                Ok(None)
            }
            Self::Status => Ok(match status()? {
                Status::On => Some("on".to_owned()),
                Status::Off => Some("off".to_owned()),
            }),
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
            "on" => Self::On,
            "off" => Self::Off,
            "toggle" => Self::Toggle,
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
