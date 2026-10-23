use std::{
    fs,
    io::{self, Read},
};

use anyhow::Result;
use camino::Utf8Path as Path;
use sha3::Digest as _;

use super::Checksum;

pub(crate) fn checksum_file(path: &Path) -> Result<Option<Checksum>> {
    match fs::File::open(path) {
        Ok(file) => Ok(Some(checksum(file)?)),
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                Ok(None)
            } else {
                Err(err.into())
            }
        }
    }
}

pub(crate) fn checksum(mut source: impl io::Read) -> io::Result<Checksum> {
    let mut buf = Vec::new();
    source.read_to_end(&mut buf)?;
    let digest = sha3::Sha3_256::digest(buf);
    Ok(Checksum(digest))
}

pub(crate) fn checksum_from(source: &[u8]) -> Checksum {
    let digest = sha3::Sha3_256::digest(source);
    Checksum(digest)
}
