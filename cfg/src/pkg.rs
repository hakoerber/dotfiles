use std::collections::HashSet;

use anyhow::{Result, anyhow};

use super::{Elevate, cmd};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallReason {
    Explicit,
    Dependency,
    DependencyIfInstalling,
}

pub async fn list_installed_packages(elevate: &Elevate) -> Result<HashSet<String>> {
    Ok(cmd::Process::run(
        "pacman",
        &["--query", "--quiet"],
        elevate,
        cmd::Stdin::Closed,
    )
    .await?
    .check()?
    .stdout
    .lines()
    .map(|line| line.to_owned())
    .collect())
}

pub async fn check_installed(name: &str, elevate: &Elevate) -> Result<bool> {
    Ok(cmd::Process::run(
        "pacman",
        &["--query", "--quiet", name],
        elevate,
        cmd::Stdin::Closed,
    )
    .await?
    .unchecked()
    .status
    .success())
}

pub async fn get_package_version(name: &str, elevate: &Elevate) -> Result<Option<String>> {
    cmd::Process::run("pacman", &["--query", name], elevate, cmd::Stdin::Closed)
        .await?
        .check()
        .ok()
        .map(|proc| {
            proc.stdout
                .split_whitespace()
                .nth(1)
                .ok_or_else(|| anyhow!("invalid pacman output for --query"))
                .map(|s| s.to_owned())
        })
        .transpose()
}

pub async fn resolve_installed_package(elevate: &Elevate, name: &str) -> Result<Option<String>> {
    // resolve until we are at the root, i.e. when the package name does not change any more
    let mut name = name.to_owned();
    loop {
        let resolved = cmd::Process::run(
            "pacman",
            &["--query", "--quiet", &name],
            elevate,
            cmd::Stdin::Closed,
        )
        .await?
        .check()
        .ok()
        .map(|output| output.stdout.trim().to_owned());

        if let Some(resolved) = resolved {
            if name == resolved {
                break Ok(Some(resolved));
            } else {
                name = resolved;
            }
        } else {
            break Ok(None);
        }
    }
}

pub async fn list_explicitly_installed_packages(elevate: &Elevate) -> Result<HashSet<String>> {
    Ok(cmd::Process::run(
        "pacman",
        &["--query", "--quiet", "--explicit"],
        elevate,
        cmd::Stdin::Closed,
    )
    .await?
    .check()?
    .stdout
    .lines()
    .map(|line| line.to_owned())
    .collect())
}

pub async fn list_explicitly_installed_packages_that_are_not_required(
    elevate: &Elevate,
) -> Result<Vec<String>> {
    Ok(cmd::Process::run(
        "pacman",
        &["--query", "--quiet", "--explicit", "--unrequired"],
        elevate,
        cmd::Stdin::Closed,
    )
    .await?
    .check()?
    .stdout
    .lines()
    .map(|line| line.to_owned())
    .collect())
}

pub async fn list_packages_installed_as_dependencies(elevate: &Elevate) -> Result<HashSet<String>> {
    Ok(cmd::Process::run(
        "pacman",
        &["--query", "--quiet", "--deps"],
        elevate,
        cmd::Stdin::Closed,
    )
    .await?
    .check()?
    .stdout
    .lines()
    .map(|line| line.to_owned())
    .collect())
}

pub async fn list_orphaned_packages(elevate: &Elevate) -> Result<HashSet<String>> {
    let pacman = cmd::Process::run(
        "pacman",
        &[
            "--query",
            "--quiet",
            "--deps",
            // `--unrequired` twice to also remove packages that are just
            // *optionally* dependencies of other packages
            "--unrequired",
            "--unrequired",
        ],
        elevate,
        cmd::Stdin::Closed,
    )
    .await?
    .unchecked();

    if pacman.stdout.is_empty() && pacman.stderr.is_empty() && pacman.status.code() == 1 {
        Ok(HashSet::new())
    } else if pacman.status.success() {
        Ok(pacman.stdout.lines().map(|line| line.to_owned()).collect())
    } else {
        Err(anyhow!(
            "invalid pacman output for orphaned packages {pacman:?}"
        ))
    }
}

pub async fn is_installed(elevate: &Elevate, name: &str) -> Result<bool> {
    Ok(cmd::Process::run(
        "pacman",
        &["--query", "--quiet", name],
        elevate,
        cmd::Stdin::Closed,
    )
    .await?
    .unchecked()
    .status
    .success())
}

pub async fn get_install_reason(elevate: &Elevate, name: &str) -> Result<Option<InstallReason>> {
    if is_installed(elevate, name).await? {
        if cmd::Process::run(
            "pacman",
            &["--query", "--quiet", "--explicit", name],
            elevate,
            cmd::Stdin::Closed,
        )
        .await?
        .unchecked()
        .status
        .success()
        {
            Ok(Some(InstallReason::Explicit))
        } else {
            Ok(Some(InstallReason::Dependency))
        }
    } else {
        Ok(None)
    }
}
