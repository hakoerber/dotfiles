#![expect(
    clippy::print_stderr,
    clippy::print_stdout,
    reason = "output is fine for cli"
)]

use std::{
    env,
    io::{self, Read as _},
    net,
    os::unix::net::UnixStream,
    process, str,
};

use thiserror::Error;

use mgr::{
    Action,
    cli::{self, CliCommand as _, ParseError},
    wire::{client, socket},
};

#[derive(Debug, Error)]
enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Socket(#[from] socket::Error),
    #[error(transparent)]
    Send(#[from] client::SendError),
    #[error(transparent)]
    CliParse(#[from] cli::ParseError),
    #[error("response is not valid utf8: {0}")]
    ResponseNonUtf8(#[from] str::Utf8Error),
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

fn main() -> MainResult {
    fn inner() -> Result<(), Error> {
        let mut args = env::args().skip(1);

        let socket = socket::get_socket_path()?;
        let mut stream = UnixStream::connect(socket)?;

        let action =
            Action::parse_str(args.next().ok_or(ParseError::MissingAction)?.as_str(), args)?;

        action.send(&mut stream)?;

        stream.shutdown(net::Shutdown::Write)?;

        let response = {
            let mut buf = Vec::new();
            stream.read_to_end(&mut buf)?;
            let response = str::from_utf8(&buf)?.to_owned();
            drop(stream);
            response
        };

        if !response.is_empty() {
            println!("{response}");
        }

        Ok(())
    }

    match inner() {
        Ok(()) => MainResult::Success,
        Err(e) => MainResult::Failure(e),
    }
}
