use std::{
    io::{self, Read, Write},
    os::unix::net::{SocketAddr, UnixListener, UnixStream},
    thread,
};

use thiserror::Error;
use tracing::{Level, event};

use super::{
    super::{Action, Exec as _},
    WireCommand, socket,
};

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error(transparent)]
    Socket(#[from] socket::Error),
    #[error(transparent)]
    Exec(#[from] crate::ExecError),
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("received unexpected eof")]
    Eof,
    #[error("received unknown byte: {0:#X}")]
    Unknown(u8),
    #[error("received surplus input: {0:?}")]
    Surplus(Vec<u8>),
    #[error("expected {expected} bytes, received only {received}")]
    MissingBytes { expected: usize, received: usize },
}

fn handle_client(stream: &mut UnixStream) -> Result<(), Error> {
    let input = {
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf)?;
        buf
    };

    event!(Level::DEBUG, "request data: {input:?}");

    let action = Action::parse_wire(input.into_iter())?;

    event!(Level::DEBUG, "parsed request: {action:?}");

    if let Some(output) = action.execute()? {
        stream.write_all(output.as_bytes())?;
    }

    Ok(())
}

pub fn run() -> Result<(), Error> {
    event!(Level::DEBUG, "starting server");

    let socket_path = socket::get_socket_path()?;

    socket::try_remove_socket(&socket_path)?;

    let socket_addr = SocketAddr::from_pathname(socket_path)?;

    event!(Level::DEBUG, "socket address {socket_addr:?}");

    let listener = UnixListener::bind_addr(&socket_addr)?;

    for stream in listener.incoming() {
        let mut stream = stream?;
        thread::spawn(move || {
            event!(Level::DEBUG, "received request");
            let result = handle_client(&mut stream);
            if let Err(e) = result {
                let msg = e.to_string();
                event!(Level::ERROR, "action failed: {msg}");
                if let Err(e) = stream.write_all(msg.as_bytes()) {
                    event!(Level::ERROR, "sending \"{msg}\" failed: {e}");
                }
            }
            event!(Level::DEBUG, "closing stream");
            drop(stream);
        });
    }

    unreachable!()
}
