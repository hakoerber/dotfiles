use thiserror::Error;
use tracing::{Level, event};

use super::{
    Exec,
    cli::{self, CliCommand},
    cmd, dmenu, dunst, spotify,
    wire::{WireCommand, server},
};

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Dunst(#[from] dunst::Error),
    #[error("unknown action: {action}")]
    UnknownAction { action: String },
    #[error(transparent)]
    Cmd(#[from] cmd::Error),
    #[error(transparent)]
    Dmenu(#[from] dmenu::Error),
    #[error(transparent)]
    Cli(#[from] cli::ParseError),
    #[error(transparent)]
    Spotify(#[from] spotify::Error),
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Menu,
    Lock,
    Suspend,
    Hibernate,
    Reboot,
    Poweroff,
}

impl Action {
    fn as_str(self) -> &'static str {
        match self {
            Self::Menu => "menu",
            Self::Lock => "lock",
            Self::Suspend => "suspend",
            Self::Hibernate => "hibernate",
            Self::Reboot => "reboot",
            Self::Poweroff => "poweroff",
        }
    }
}

impl WireCommand for Action {
    fn parse_wire(mut input: impl Iterator<Item = u8>) -> Result<Self, server::ParseError> {
        match input.next().ok_or(server::ParseError::Eof)? {
            0x01 => Ok(Self::Menu),
            0x02 => Ok(Self::Lock),
            0x03 => Ok(Self::Suspend),
            0x04 => Ok(Self::Hibernate),
            0x05 => Ok(Self::Reboot),
            0x06 => Ok(Self::Poweroff),
            byte => Err(server::ParseError::Unknown(byte)),
        }
    }

    fn to_wire(&self) -> Vec<u8> {
        match *self {
            Self::Menu => vec![0x01],
            Self::Lock => vec![0x02],
            Self::Suspend => vec![0x03],
            Self::Hibernate => vec![0x04],
            Self::Reboot => vec![0x05],
            Self::Poweroff => vec![0x06],
        }
    }
}

impl Exec for Action {
    type ExecErr = Error;

    fn execute(&self) -> Result<Option<String>, Self::ExecErr> {
        match *self {
            Self::Menu => menu()?,
            Self::Lock => lock_and_screen_off()?,
            Self::Suspend => lock_and_suspend()?,
            Self::Hibernate => hibernate()?,
            Self::Reboot => reboot()?,
            Self::Poweroff => poweroff()?,
        }
        Ok(None)
    }
}

impl CliCommand for Action {
    type ExecErr = Error;

    fn parse_str(input: &str, rest: impl Iterator<Item = String>) -> Result<Self, cli::ParseError>
    where
        Self: Sized,
    {
        let result = match input {
            "menu" => Self::Menu,
            "lock" => Self::Lock,
            "suspend" => Self::Suspend,
            "hibernate" => Self::Hibernate,
            "reboot" => Self::Reboot,
            "shutdown" => Self::Poweroff,
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

const MENU_ACTIONS: &[Action] = &[
    Action::Lock,
    Action::Suspend,
    Action::Hibernate,
    Action::Reboot,
    Action::Poweroff,
];

fn menu() -> Result<(), Error> {
    let choice = dmenu::get_choice(
        &MENU_ACTIONS
            .iter()
            .map(|action| action.as_str())
            .collect::<Vec<&str>>(),
    )?;

    if let Some(choice) = choice {
        MENU_ACTIONS
            .iter()
            .find(|action| action.as_str() == choice)
            .copied()
            .expect("choice must be one of the valid values")
            .execute()?;
    } else {
        event!(Level::DEBUG, "rofi was cancelled");
    }

    Ok(())
}

fn screen_off() -> Result<(), Error> {
    Ok(cmd::command("xset", &["dpms", "force", "off"])?)
}

fn lock() -> Result<cmd::RunningProcess, Error> {
    match spotify::pause() {
        Ok(_) => (),
        Err(spotify::Error::NotFound) => (),
        Err(e) => return Err(e.into()),
    }

    let lock_handle = cmd::start_command(
        "i3lock",
        &[
            "--nofork",
            "--show-failed-attempts",
            "--ignore-empty-password",
            "--color",
            "000000",
        ],
    );

    Ok(lock_handle)
}

fn reset_screen() -> Result<(), Error> {
    Ok(cmd::command(
        "systemctl",
        &["--user", "restart", "dpms.service"],
    )?)
}

fn lock_and_screen_off() -> Result<(), Error> {
    let dunst_paused = dunst::is_paused()?;
    if dunst_paused {
        dunst::set_status(dunst::Status::Paused)?;
    }

    lock()?.with(|| -> Result<(), Error> {
        screen_off()?;
        Ok(())
    })?;

    if dunst_paused {
        dunst::set_status(dunst::Status::Unpaused)?;
    }

    reset_screen()?;

    Ok(())
}

fn suspend() -> Result<(), Error> {
    Ok(cmd::command("systemctl", &["suspend"])?)
}

fn hibernate() -> Result<(), Error> {
    Ok(cmd::command("systemctl", &["hibernate"])?)
}

fn reboot() -> Result<(), Error> {
    Ok(cmd::command("systemctl", &["reboot"])?)
}

fn poweroff() -> Result<(), Error> {
    Ok(cmd::command("systemctl", &["poweroff"])?)
}

fn lock_and_suspend() -> Result<(), Error> {
    lock()?.with(|| -> Result<(), Error> {
        screen_off()?;
        suspend()?;
        Ok(())
    })?;

    Ok(())
}
