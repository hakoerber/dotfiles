use std::fmt;

use sha3::Digest as _;

#[derive(Eq, PartialEq, Debug)]
pub struct Checksum(
    sha3::digest::array::Array<u8, <sha3::Sha3_256 as sha3::digest::OutputSizeUser>::OutputSize>,
);

impl fmt::Display for Checksum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0.as_slice() {
            write!(f, "{byte:x}")?;
        }

        Ok(())
    }
}

pub(crate) fn checksum_from(source: &[u8]) -> Checksum {
    let digest = sha3::Sha3_256::digest(source);
    Checksum(digest)
}
