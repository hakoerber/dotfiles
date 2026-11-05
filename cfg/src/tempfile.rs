//! Would be cool to be able to pass the droppable struct through a typestate
//! pipeline, but this looks like it's not easily possible. Maybe have a ref
//! inside each struct that implements `Drop`?

use std::future::AsyncDrop;

use anyhow::Result;
use camino::Utf8Path as Path;

use super::{Elevate, PathBuf, cmd, fs};

#[must_use]
#[derive(Debug)]
pub(crate) struct Tempfile<'elevate> {
    path: Option<PathBuf>,
    elevate: &'elevate Elevate,
}

impl AsyncDrop for Tempfile<'_> {
    async fn drop(self: std::pin::Pin<&mut Self>) {
        if let Some(ref path) = self.path {
            fs::remove_file(self.elevate, path)
                .await
                .expect("cleanup failed");
        }
    }
}

impl Drop for Tempfile<'_> {
    fn drop(&mut self) {
        if let Some(ref path) = self.path {
            fs::remove_file_sync(self.elevate, path).expect("cleanup failed");
        }
    }
}

impl<'a> Tempfile<'a> {
    pub(crate) async fn new(elevate: &'a Elevate) -> Result<Self> {
        let tmpfile = PathBuf::from(
            cmd::Process::run("mktemp", &["--tmpdir=/var/tmp"], elevate, None)
                .await?
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

    pub(crate) async fn write(&self, input: &[u8]) -> Result<()> {
        fs::write(
            self.path.as_ref().expect("writing to closed tempfile"),
            input,
            self.elevate,
        )
        .await?;
        Ok(())
    }

    pub(crate) fn path(&self) -> &Path {
        self.path
            .as_ref()
            .expect("getting path of removed tempfile")
    }

    async fn remove(mut self) -> Result<()> {
        Box::pin(fs::remove_file(
            self.elevate,
            &self.path.take().expect("removing already removed tempfile"),
        ))
        .await
    }

    pub(crate) async fn read_all(self) -> Result<Vec<u8>> {
        let path = self.path.as_ref().expect("saving a removed tempfile");
        let content = fs::read(self.elevate, path).await?;
        self.remove().await?;
        Ok(content)
    }

    pub(crate) async fn save_as(mut self, to: &Path) -> Result<()> {
        let path = self.path.take().expect("saving a removed tempfile");
        fs::mv(&path, to, self.elevate).await?;
        Ok(())
    }
}
