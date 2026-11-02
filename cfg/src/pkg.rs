use anyhow::{Result, anyhow};

use super::{Elevate, cmd};

pub fn list_installed_packages(elevate: &Elevate) -> Result<Vec<String>> {
    Ok(
        cmd::Process::run("pacman", &["--query", "--quiet"], elevate, None)?
            .check()?
            .stdout
            .lines()
            .map(|line| line.to_owned())
            .collect(),
    )
}

pub fn check_installed(name: &str, elevate: &Elevate) -> Result<bool> {
    Ok(
        cmd::Process::run("pacman", &["--query", "--quiet", name], elevate, None)?
            .unchecked()
            .status
            .success(),
    )
}

pub fn list_explicitly_installed_packages(elevate: &Elevate) -> Result<Vec<String>> {
    Ok(cmd::Process::run(
        "pacman",
        &["--query", "--quiet", "--explicit"],
        elevate,
        None,
    )?
    .check()?
    .stdout
    .lines()
    .map(|line| line.to_owned())
    .collect())
}

pub fn list_orphaned_packages(elevate: &Elevate) -> Result<Vec<String>> {
    let pacman = cmd::Process::run(
        "pacman",
        &["--query", "--quiet", "--deps", "--unrequired"],
        elevate,
        None,
    )?
    .unchecked();

    if pacman.stdout.is_empty() && pacman.stderr.is_empty() && pacman.status.code() == 1 {
        Ok(vec![])
    } else {
        if pacman.status.success() {
            Ok(pacman.stdout.lines().map(|line| line.to_owned()).collect())
        } else {
            Err(anyhow!(
                "invalid pacman output for orphaned packages {pacman:?}"
            ))
        }
    }
}
