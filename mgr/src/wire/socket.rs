use std::{
    fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use super::super::dirs;

#[derive(Debug, Error)]
pub enum Error {
    #[error("could not find a suitable socket path")]
    NoSocketPathFound,
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Dirs(#[from] dirs::Error),
}

pub fn get_socket_path() -> Result<PathBuf, Error> {
    if let Some(mut dir) = dirs::xdg_runtime_dir()? {
        dir.push("workstation-mgr.sock");
        return Ok(dir);
    }

    Err(Error::NoSocketPathFound)
}

pub(crate) fn try_remove_socket(path: &Path) -> Result<(), Error> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
