use thiserror::Error;

pub(crate) mod brightness;
pub mod cli;
pub(crate) mod cmd;
pub(crate) mod dirs;
pub mod dmenu;
pub(crate) mod dunst;
pub(crate) mod env;
pub(crate) mod gammastep;
pub(crate) mod pipewire;
pub mod power;
pub mod present;
pub(crate) mod sleep;
pub(crate) mod spotify;
pub(crate) mod systemd;
pub(crate) mod weather;
pub mod wire;

#[derive(Debug, Error)]
pub enum ExecError {
    #[error(transparent)]
    Power(#[from] power::Error),
    #[error(transparent)]
    Presentation(#[from] present::Error),
    #[error(transparent)]
    Pipewire(#[from] pipewire::Error),
    #[error(transparent)]
    Spotify(#[from] spotify::Error),
    #[error(transparent)]
    Gammastep(#[from] gammastep::Error),
    #[error(transparent)]
    Weather(#[from] weather::Error),
    #[error(transparent)]
    Brightness(#[from] brightness::Error),
    #[error(transparent)]
    Sleep(#[from] sleep::Error),
    #[error(transparent)]
    Parse(#[from] cli::ParseError),
}

#[derive(Debug)]
pub enum Action {
    Power(power::Action),
    Present(present::Action),
    Pipewire(pipewire::Action),
    Spotify(spotify::Action),
    Gammastep(gammastep::Action),
    Weather(weather::Action),
    Brightness(brightness::Action),
    Sleep(sleep::Action),
}

impl wire::WireCommand for Action {
    fn parse_wire(mut input: impl Iterator<Item = u8>) -> Result<Self, wire::server::ParseError> {
        match input.next().ok_or(wire::server::ParseError::Eof)? {
            0x01 => Ok(Self::Power(power::Action::parse_wire(input)?)),
            0x02 => Ok(Self::Present(present::Action::parse_wire(input)?)),
            0x03 => Ok(Self::Pipewire(pipewire::Action::parse_wire(input)?)),
            // 0x04 was used by the theme switcher
            0x05 => Ok(Self::Spotify(spotify::Action::parse_wire(input)?)),
            0x06 => Ok(Self::Gammastep(gammastep::Action::parse_wire(input)?)),
            0x07 => Ok(Self::Weather(weather::Action::parse_wire(input)?)),
            0x08 => Ok(Self::Brightness(brightness::Action::parse_wire(input)?)),
            0x09 => Ok(Self::Sleep(sleep::Action::parse_wire(input)?)),
            other => Err(wire::server::ParseError::Unknown(other)),
        }
    }

    fn to_wire(&self) -> Vec<u8> {
        match *self {
            Self::Power(action) => {
                let mut v = vec![0x01];
                v.extend_from_slice(&action.to_wire());
                v
            }
            Self::Present(action) => {
                let mut v = vec![0x02];
                v.extend_from_slice(&action.to_wire());
                v
            }
            Self::Pipewire(action) => {
                let mut v = vec![0x03];
                v.extend_from_slice(&action.to_wire());
                v
            }
            // 0x04 was used by the theme switcher
            Self::Spotify(action) => {
                let mut v = vec![0x05];
                v.extend_from_slice(&action.to_wire());
                v
            }
            Self::Gammastep(action) => {
                let mut v = vec![0x06];
                v.extend_from_slice(&action.to_wire());
                v
            }
            Self::Weather(action) => {
                let mut v = vec![0x07];
                v.extend_from_slice(&action.to_wire());
                v
            }
            Self::Brightness(action) => {
                let mut v = vec![0x08];
                v.extend_from_slice(&action.to_wire());
                v
            }
            Self::Sleep(action) => {
                let mut v = vec![0x09];
                v.extend_from_slice(&action.to_wire());
                v
            }
        }
    }
}

impl cli::CliCommand for Action {
    type ExecErr = ExecError;

    fn parse_str(
        input: &str,
        mut rest: impl Iterator<Item = String>,
    ) -> Result<Self, cli::ParseError>
    where
        Self: Sized,
    {
        match input {
            "power" => {
                let choice = rest.next().ok_or(cli::ParseError::MissingAction)?;
                Ok(Self::Power(power::Action::parse_str(&choice, rest)?))
            }
            "present" => {
                let choice = rest.next().ok_or(cli::ParseError::MissingAction)?;
                Ok(Self::Present(present::Action::parse_str(&choice, rest)?))
            }
            "pipewire" => {
                let choice = rest.next().ok_or(cli::ParseError::MissingAction)?;
                Ok(Self::Pipewire(pipewire::Action::parse_str(&choice, rest)?))
            }
            "spotify" => {
                let choice = rest.next().ok_or(cli::ParseError::MissingAction)?;
                Ok(Self::Spotify(spotify::Action::parse_str(&choice, rest)?))
            }
            "gammastep" => {
                let choice = rest.next().ok_or(cli::ParseError::MissingAction)?;
                Ok(Self::Gammastep(gammastep::Action::parse_str(
                    &choice, rest,
                )?))
            }
            "weather" => {
                let choice = rest.next().ok_or(cli::ParseError::MissingAction)?;
                Ok(Self::Weather(weather::Action::parse_str(&choice, rest)?))
            }
            "brightness" => {
                let choice = rest.next().ok_or(cli::ParseError::MissingAction)?;
                Ok(Self::Brightness(brightness::Action::parse_str(
                    &choice, rest,
                )?))
            }
            "sleep" => {
                let choice = rest.next().ok_or(cli::ParseError::MissingAction)?;
                Ok(Self::Sleep(sleep::Action::parse_str(&choice, rest)?))
            }
            s => Err(cli::ParseError::UnknownAction {
                action: s.to_owned(),
            }),
        }
    }
}

pub trait Exec {
    type ExecErr: Into<ExecError>;

    fn execute(&self) -> Result<Option<String>, Self::ExecErr>;
}

impl Exec for Action {
    type ExecErr = ExecError;

    fn execute(&self) -> Result<Option<String>, Self::ExecErr> {
        match *self {
            Self::Power(action) => Ok(action.execute()?),
            Self::Present(action) => Ok(action.execute()?),
            Self::Pipewire(action) => Ok(action.execute()?),
            Self::Spotify(action) => Ok(action.execute()?),
            Self::Gammastep(action) => Ok(action.execute()?),
            Self::Weather(action) => Ok(action.execute()?),
            Self::Brightness(action) => Ok(action.execute()?),
            Self::Sleep(action) => Ok(action.execute()?),
        }
    }
}
