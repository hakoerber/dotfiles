use std::{env, io::Write as _};

use anyhow::Result;
use camino::Utf8Path as Path;
use tempfile::NamedTempFile;

pub(crate) struct Tempfile(NamedTempFile);

impl Tempfile {
    pub(crate) fn new_in(dir: &Path) -> Result<Self> {
        let file = tempfile::NamedTempFile::new_in(dir)?;
        Ok(Tempfile(file))
    }

    pub(crate) fn path(&self) -> &Path {
        self.0.path().try_into().expect("utf-8 paths")
    }

    pub(crate) fn write_all(&mut self, input: &[u8]) -> Result<()> {
        self.0.write_all(input)?;
        Ok(())
    }
}
