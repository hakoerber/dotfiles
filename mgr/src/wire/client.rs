use std::{
    io::{self, Write as _},
    os::unix::net::UnixStream,
};

use thiserror::Error;

use super::{super::Action, WireCommand as _};

#[derive(Debug, Error)]
pub enum SendError {
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl Action {
    pub fn send(&self, stream: &mut UnixStream) -> Result<(), SendError> {
        Ok(stream.write_all(&self.to_wire())?)
    }
}
