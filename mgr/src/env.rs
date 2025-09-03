use std::{env, ffi::OsString};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(
        "env variable \"{name}\" is not valid unicode: \"{value}\"",
        value = value.to_string_lossy()
    )]
    EnvNotUnicode { name: &'static str, value: OsString },
    #[error("env variable \"{name}\" not found")]
    EnvNotFound { name: &'static str },
}

pub(crate) fn get(var: &'static str) -> Result<Option<String>, Error> {
    match env::var(var) {
        Ok(value) => Ok(Some(value)),
        Err(e) => match e {
            env::VarError::NotPresent => Ok(None),
            env::VarError::NotUnicode(value) => Err(Error::EnvNotUnicode { name: var, value }),
        },
    }
}

pub(crate) fn require(var: &'static str) -> Result<String, Error> {
    get(var)?.ok_or(Error::EnvNotFound { name: var })
}
