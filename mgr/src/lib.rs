use thiserror::Error;

pub(crate) mod brightness;
pub mod cli;
pub(crate) mod cmd;
pub(crate) mod dirs;
pub mod dmenu;
pub(crate) mod dunst;
pub(crate) mod env;
pub mod power;
pub mod present;
pub(crate) mod pulseaudio;
pub(crate) mod redshift;
pub(crate) mod spotify;
pub(crate) mod systemd;
pub(crate) mod theme;
pub(crate) mod weather;
pub mod wire;

#[derive(Debug, Error)]
pub enum ExecError {
    #[error(transparent)]
    Power(#[from] power::Error),
    #[error(transparent)]
    Presentation(#[from] present::Error),
    #[error(transparent)]
    Pulseaudio(#[from] pulseaudio::Error),
    #[error(transparent)]
    Theme(#[from] theme::Error),
    #[error(transparent)]
    Spotify(#[from] spotify::Error),
    #[error(transparent)]
    Redshift(#[from] redshift::Error),
    #[error(transparent)]
    Weather(#[from] weather::Error),
    #[error(transparent)]
    Brightness(#[from] brightness::Error),
    #[error(transparent)]
    Parse(#[from] cli::ParseError),
}

#[derive(Debug)]
pub enum Action {
    Power(power::Action),
    Present(present::Action),
    Pulseaudio(pulseaudio::Action),
    Theme(theme::Action),
    Spotify(spotify::Action),
    Redshift(redshift::Action),
    Weather(weather::Action),
    Brightness(brightness::Action),
}

impl wire::WireCommand for Action {
    fn parse_wire(mut input: impl Iterator<Item = u8>) -> Result<Self, wire::server::ParseError> {
        match input.next().ok_or(wire::server::ParseError::Eof)? {
            0x01 => Ok(Self::Power(power::Action::parse_wire(input)?)),
            0x02 => Ok(Self::Present(present::Action::parse_wire(input)?)),
            0x03 => Ok(Self::Pulseaudio(pulseaudio::Action::parse_wire(input)?)),
            0x04 => Ok(Self::Theme(theme::Action::parse_wire(input)?)),
            0x05 => Ok(Self::Spotify(spotify::Action::parse_wire(input)?)),
            0x06 => Ok(Self::Redshift(redshift::Action::parse_wire(input)?)),
            0x07 => Ok(Self::Weather(weather::Action::parse_wire(input)?)),
            0x08 => Ok(Self::Brightness(brightness::Action::parse_wire(input)?)),
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
            Self::Pulseaudio(action) => {
                let mut v = vec![0x03];
                v.extend_from_slice(&action.to_wire());
                v
            }
            Self::Theme(action) => {
                let mut v = vec![0x04];
                v.extend_from_slice(&action.to_wire());
                v
            }
            Self::Spotify(action) => {
                let mut v = vec![0x05];
                v.extend_from_slice(&action.to_wire());
                v
            }
            Self::Redshift(action) => {
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
            "pulseaudio" => {
                let choice = rest.next().ok_or(cli::ParseError::MissingAction)?;
                Ok(Self::Pulseaudio(pulseaudio::Action::parse_str(
                    &choice, rest,
                )?))
            }
            "theme" => {
                let choice = rest.next().ok_or(cli::ParseError::MissingAction)?;
                Ok(Self::Theme(theme::Action::parse_str(&choice, rest)?))
            }
            "spotify" => {
                let choice = rest.next().ok_or(cli::ParseError::MissingAction)?;
                Ok(Self::Spotify(spotify::Action::parse_str(&choice, rest)?))
            }
            "redshift" => {
                let choice = rest.next().ok_or(cli::ParseError::MissingAction)?;
                Ok(Self::Redshift(redshift::Action::parse_str(&choice, rest)?))
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
            Self::Pulseaudio(action) => Ok(action.execute()?),
            Self::Theme(action) => Ok(action.execute()?),
            Self::Spotify(action) => Ok(action.execute()?),
            Self::Redshift(action) => Ok(action.execute()?),
            Self::Weather(action) => Ok(action.execute()?),
            Self::Brightness(action) => Ok(action.execute()?),
        }
    }
}
