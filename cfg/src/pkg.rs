use anyhow::Result;

use super::{Elevate, cmd};

pub(crate) fn list_installed_packages(elevate: &Elevate) -> Result<Vec<String>> {
    Ok(
        cmd::Process::run("pacman", &["--query", "--quiet"], elevate, None)?
            .check()?
            .stdout
            .lines()
            .map(|line| line.to_owned())
            .collect(),
    )
}
