#![expect(
    clippy::print_stderr,
    clippy::print_stdout,
    reason = "output is fine for cli"
)]

use std::{env, process};

use thiserror::Error;
use tracing::Level;

use mgr::{
    self, Action, Exec as _,
    cli::{CliCommand as _, ParseError},
};

#[derive(Debug, Error)]
enum Error {
    #[error(transparent)]
    Power(#[from] mgr::power::Error),
    #[error(transparent)]
    Dmenu(#[from] mgr::dmenu::Error),
    #[error(transparent)]
    Server(#[from] mgr::wire::server::Error),
    #[error(transparent)]
    Presentation(#[from] mgr::present::Error),
    #[error(transparent)]
    Exec(#[from] mgr::ExecError),
    #[error(transparent)]
    ParseParse(#[from] ParseError),
    #[error(transparent)]
    Tracing(#[from] tracing::dispatcher::SetGlobalDefaultError),
}

enum MainResult {
    Success,
    Failure(Error),
}

impl process::Termination for MainResult {
    fn report(self) -> process::ExitCode {
        match self {
            Self::Success => process::ExitCode::SUCCESS,
            Self::Failure(e) => {
                eprintln!("Error: {e}");
                process::ExitCode::FAILURE
            }
        }
    }
}

impl From<Error> for MainResult {
    fn from(value: Error) -> Self {
        Self::Failure(value)
    }
}

fn init_tracing() -> Result<(), Error> {
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_max_level(Level::DEBUG)
            .event_format(
                tracing_subscriber::fmt::format()
                    .with_ansi(false)
                    .with_target(false)
                    .compact(),
            )
            .finish(),
    )?;
    Ok(())
}

fn main() -> MainResult {
    fn inner() -> Result<(), Error> {
        init_tracing()?;

        let mut args = env::args().skip(1);

        match args.next().ok_or(ParseError::MissingAction)?.as_str() {
            "serve" => {
                mgr::wire::server::run()?;
                Ok(())
            }
            "run" => {
                let action = Action::parse_str(
                    args.next().ok_or(ParseError::MissingAction)?.as_str(),
                    args,
                )?;
                if let Some(output) = action.execute()? {
                    println!("{output}");
                }
                Ok(())
            }
            input => Err(ParseError::UnknownAction {
                action: input.to_owned(),
            }
            .into()),
        }
    }

    match inner() {
        Ok(()) => MainResult::Success,
        Err(e) => MainResult::Failure(e),
    }
}
