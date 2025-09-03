use std::path::PathBuf;

use thiserror::Error;

use super::env;

const ENV_XDG_RUNTIME_DIR: &str = "XDG_RUNTIME_DIR";
const ENV_XDG_CACHE_DIR: &str = "XDG_CACHE_HOME";

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Env(#[from] env::Error),
}

pub(crate) fn xdg_runtime_dir() -> Result<Option<PathBuf>, Error> {
    Ok(env::get(ENV_XDG_RUNTIME_DIR)?.map(PathBuf::from))
}

pub(crate) fn require_xdg_runtime_dir() -> Result<PathBuf, Error> {
    Ok(PathBuf::from(env::require(ENV_XDG_RUNTIME_DIR)?))
}

pub(crate) fn xdg_cache_dir() -> Result<PathBuf, Error> {
    Ok(match env::get(ENV_XDG_CACHE_DIR)? {
        Some(value) => PathBuf::from(value),
        None => PathBuf::from(env::require("HOME")?).join(".cache"),
    })
}
