use std::{
    fs, io,
    ops::Sub as _,
    path::{Path, PathBuf},
};

use thiserror::Error;
use time::format_description::well_known::Iso8601;
use tracing::{Level, event};

const CACHE_AGE: time::Duration = time::Duration::hours(1);
const CACHE_DELIMITER: char = '|';

use super::{
    Exec,
    cli::{self, CliCommand},
    cmd, dirs, systemd,
    wire::{WireCommand, server},
};

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Cmd(#[from] cmd::Error),
    #[error(transparent)]
    Cli(#[from] cli::ParseError),
    #[error(transparent)]
    Systemd(#[from] systemd::Error),
    #[error(transparent)]
    Http(#[from] ureq::Error),
    #[error(transparent)]
    Dirs(#[from] dirs::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("delimiter not found in cache file")]
    CacheDelimitedNotFound,
    #[error("invalid timestamp \"{input}\" in cache file: {error}")]
    CacheTimestampParse {
        input: String,
        error: time::error::Parse,
    },
    #[error("cache timestamp ({cache_timestamp}) is from the future (now: {now})")]
    CacheTimestampOverflow {
        now: time::UtcDateTime,
        cache_timestamp: time::UtcDateTime,
    },
    #[error("formatting cache timestamp failed: {error}")]
    CacheTimestampFormat { error: time::error::Format },
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Get,
}

impl WireCommand for Action {
    fn parse_wire(mut input: impl Iterator<Item = u8>) -> Result<Self, server::ParseError> {
        match input.next().ok_or(server::ParseError::Eof)? {
            0x01 => Ok(Self::Get),
            byte => Err(server::ParseError::Unknown(byte)),
        }
    }

    fn to_wire(&self) -> Vec<u8> {
        match *self {
            Self::Get => vec![0x01],
        }
    }
}

impl Exec for Action {
    type ExecErr = Error;

    fn execute(&self) -> Result<Option<String>, Self::ExecErr> {
        match *self {
            Self::Get => Ok(Some(get()?)),
        }
    }
}

impl CliCommand for Action {
    type ExecErr = Error;

    fn parse_str(input: &str, rest: impl Iterator<Item = String>) -> Result<Self, cli::ParseError>
    where
        Self: Sized,
    {
        let result = match input {
            "get" => Self::Get,
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

#[derive(Debug)]
struct Cache {
    timestamp: time::UtcDateTime,
    value: String,
}

fn cache_file() -> Result<PathBuf, Error> {
    Ok(dirs::xdg_cache_dir()?.join("workstation-mgr.wttr.cache"))
}

fn store_cache(path: &Path, timestamp: &time::UtcDateTime, value: &str) -> Result<(), Error> {
    event!(Level::DEBUG, "storing in cache: {timestamp} {value}");
    Ok(fs::write(
        path,
        format!(
            "{timestamp}{CACHE_DELIMITER}{value}",
            timestamp = timestamp
                .format(&Iso8601::DEFAULT)
                .map_err(|error| Error::CacheTimestampFormat { error })?,
        ),
    )?)
}

fn get_cache(path: &Path) -> Result<Option<Cache>, Error> {
    match fs::read_to_string(path) {
        Ok(content) => {
            let (timestamp, value) = content
                .split_once(CACHE_DELIMITER)
                .ok_or(Error::CacheDelimitedNotFound)?;

            let cache_timestamp =
                time::UtcDateTime::parse(timestamp, &Iso8601::DEFAULT).map_err(|error| {
                    Error::CacheTimestampParse {
                        input: timestamp.to_owned(),
                        error,
                    }
                })?;

            Ok(Some(Cache {
                timestamp: cache_timestamp,
                value: value.to_owned(),
            }))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn request() -> Result<String, Error> {
    Ok(ureq::get("https://wttr.in/Ansbach?m&T&format=%c%t")
        .call()?
        .body_mut()
        .read_to_string()?)
}

fn get_and_update_cache(cache_file: &Path, now: &time::UtcDateTime) -> Result<String, Error> {
    event!(Level::DEBUG, "refreshing cache");
    let value = request()?;
    store_cache(cache_file, now, &value)?;
    Ok(value)
}

fn get() -> Result<String, Error> {
    let cache_file = cache_file()?;
    event!(Level::DEBUG, "using cache file {cache_file:?}");

    let cache = get_cache(&cache_file)?;
    event!(Level::DEBUG, "read from cache: {cache:?}");

    let now = time::UtcDateTime::now();

    match cache {
        Some(cache) => {
            let cache_age = now.sub(cache.timestamp);
            event!(Level::DEBUG, "cache age: {cache_age}");

            if cache_age.is_negative() {
                return Err(Error::CacheTimestampOverflow {
                    now,
                    cache_timestamp: cache.timestamp,
                });
            }

            if cache_age <= CACHE_AGE {
                event!(Level::DEBUG, "reusing cache");
                Ok(cache.value)
            } else {
                get_and_update_cache(&cache_file, &now)
            }
        }
        None => get_and_update_cache(&cache_file, &now),
    }
}
