pub mod checksum;
pub mod cmd;
pub mod fs;
pub mod gpg;
pub mod pkg;
pub mod tempfile;
pub mod user;

pub use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};

#[derive(Eq, PartialEq, Debug)]
pub enum ElevateTo {
    Root,
    User { user: String },
}

#[derive(Eq, PartialEq, Debug)]
pub enum Elevate {
    Sudo(ElevateTo),
    No,
}
