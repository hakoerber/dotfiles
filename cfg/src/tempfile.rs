//! Would be cool to be able to pass the droppable struct through a typestate
//! pipeline, but this looks like it's not easily possible. Maybe have a ref
//! inside each struct that implements `Drop`?

use anyhow::Result;
use camino::Utf8Path as Path;

use super::{Elevate, PathBuf, cmd, fs};

#[must_use]
#[derive(Debug)]
pub(crate) struct Tempfile<'ctx> {
    path: Option<PathBuf>,
    elevate: &'ctx Elevate,
}

impl Drop for Tempfile<'_> {
    fn drop(&mut self) {
        if let Some(ref path) = self.path {
            fs::remove_file(self.elevate, path).expect("cleanup failed");
        }
    }
}

impl<'a> Tempfile<'a> {
    pub(crate) fn new(elevate: &'a Elevate) -> Result<Self> {
        let tmpfile = PathBuf::from(
            cmd::Process::run("mktemp", &["--tmpdir=/var/tmp"], elevate, None)?
                .check()?
                .stdout
                .trim()
                .to_owned(),
        );

        Ok(Self {
            path: Some(tmpfile),
            elevate,
        })
    }

    pub(crate) fn write(&self, input: &[u8]) -> Result<()> {
        fs::write(
            self.path.as_ref().expect("writing to closed tempfile"),
            input,
            self.elevate,
        )?;
        Ok(())
    }

    pub(crate) fn path(&self) -> &Path {
        self.path
            .as_ref()
            .expect("getting path of removed tempfile")
    }

    fn remove(mut self) -> Result<()> {
        fs::remove_file(
            self.elevate,
            &self.path.take().expect("removing already removed tempfile"),
        )
    }

    pub(crate) fn read_all(self) -> Result<Vec<u8>> {
        let path = self.path.as_ref().expect("saving a removed tempfile");
        let content = fs::read(self.elevate, path)?;
        self.remove()?;
        Ok(content)
    }

    pub(crate) fn save_as(mut self, to: &Path) -> Result<()> {
        let path = self.path.take().expect("saving a removed tempfile");
        fs::mv(&path, to, self.elevate)?;
        Ok(())
    }
}
