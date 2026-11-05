use std::{
    cell::{LazyCell, OnceCell},
    sync::{LazyLock, OnceLock},
};

use anyhow::Result;
use nonempty::NonEmpty;
use tap::Tap as _;

use crate::{
    maybeowned::MaybeOwned,
    pkg::{list_explicitly_installed_packages, list_installed_packages},
};

use super::super::{Applyable, ChangeStatus, Elevate, Plannable, Reporter, cmd};

static INSTALLED_PACKAGES: OnceLock<Vec<String>> = OnceLock::new();

fn installed_packages(elevate: &Elevate) -> Result<&Vec<String>> {
    let packages = INSTALLED_PACKAGES.get_or_try_init(|| list_installed_packages(elevate))?;
    Ok(packages)
}

static EXPLICITLY_INSTALLED_PACKAGES: OnceLock<Vec<String>> = OnceLock::new();

fn explicitly_installed_packages(elevate: &Elevate) -> Result<&Vec<String>> {
    let packages = EXPLICITLY_INSTALLED_PACKAGES
        .get_or_try_init(|| list_explicitly_installed_packages(elevate))?;
    Ok(packages)
}

#[derive(Debug)]
pub struct PackageDatabaseRefresh;

impl<'change> Plannable<'change> for PackageDatabaseRefresh {
    fn plan<'elevate>(self: Box<Self>, elevate: &'elevate Elevate) -> Result<ChangeStatus<'change>>
    where
        'elevate: 'change,
    {
        Ok(ChangeStatus::NeedsChange(Box::new(
            PackageDatabaseRefreshPlan { args: &() },
        )))
    }
}

#[derive(Debug)]
pub(crate) struct PackageDatabaseRefreshPlan<'a> {
    // #[expect(unused, reason = "kept to have the same shape as other plans")]
    pub(crate) args: &'a (),
}

impl Applyable for PackageDatabaseRefreshPlan<'_> {
    fn apply(&self, elevate: &Elevate) -> Result<()> {
        cmd::Process::run("pacman", &["--sync", "--refresh"], elevate, None)?.check()?;
        Ok(())
    }
}

impl Reporter for PackageDatabaseRefreshPlan<'_> {
    fn report(&self) -> Result<Vec<String>> {
        Ok(vec!["package database refresh".to_owned()])
    }
}

#[derive(Debug, Clone, Copy)]
pub enum InstallReason {
    Explicit,
    Dependency,
}

#[derive(Debug)]
pub struct Packages<'action> {
    pub install: MaybeOwned<'action, [MaybeOwned<'action, str>]>,
    pub reason: InstallReason,
}

impl<'change> Plannable<'change> for Packages<'change> {
    fn plan<'elevate>(self: Box<Self>, elevate: &'elevate Elevate) -> Result<ChangeStatus<'change>>
    where
        'elevate: 'change,
    {
        let mut install = Vec::new();

        for package in self.install.as_ref() {
            if !installed_packages(elevate)?
                .iter()
                .any(|s| s == package.as_ref())
            {
                install.push(package.clone());
            }
        }

        let mut set_reason = Vec::new();

        match self.reason {
            InstallReason::Explicit => {
                for package in self.install.as_ref() {
                    if !explicitly_installed_packages(elevate)?
                        .iter()
                        .any(|s| s == package.as_ref())
                    {
                        set_reason.push(package.clone());
                    }
                }
            }
            InstallReason::Dependency => {
                for package in self.install.as_ref() {
                    if !packages_installed_as_dependencies(elevate)?
                        .iter()
                        .any(|s| s == package.as_ref())
                    {
                        set_reason.push(package.clone());
                    }
                }
            }
        }

        if !install.is_empty() || !set_reason.is_empty() {
            Ok(ChangeStatus::NeedsChange(Box::new(PackagePlan {
                set_reason,
                args: *self,
                install,
            })))
        } else {
            Ok(ChangeStatus::Clean)
        }
    }
}

#[derive(Debug)]
pub(crate) struct PackagePlan<'a> {
    // #[expect(unused, reason = "kept to have the same shape as other plans")]
    pub(crate) args: Packages<'a>,
    pub(crate) install: Vec<MaybeOwned<'a, str>>,
    pub(crate) set_reason: Vec<MaybeOwned<'a, str>>,
}

impl Applyable for PackagePlan<'_> {
    fn apply(&self, elevate: &Elevate) -> Result<()> {
        if !self.install.is_empty() {
            cmd::Process::run(
                "pacman",
                &vec!["--needed", "--noconfirm", "--sync", "--"]
                    .tap_mut(|v| v.extend(self.install.iter().map(|i| i.as_ref()))),
                elevate,
                None,
            )?
            .check()?;
        }

        if !self.set_reason.is_empty() {
            cmd::Process::run(
                "pacman",
                &vec![
                    "--database",
                    match self.args.reason {
                        InstallReason::Explicit => "--asexplicit",
                        InstallReason::Dependency => "--asdeps",
                    },
                    "--",
                ]
                .tap_mut(|v| v.extend(self.set_reason.iter().map(|i| i.as_ref()))),
                elevate,
                None,
            )?
            .check()?;
        }

        Ok(())
    }
}

impl Reporter for PackagePlan<'_> {
    fn report(&self) -> Result<Vec<String>> {
        let mut out = vec![];
        for install in &self.install {
            out.push(format!("install {name}", name = install.as_ref()));
        }
        for package in &self.set_reason {
            out.push(format!(
                "set {name} as {message}",
                name = package.as_ref(),
                message = match self.args.reason {
                    InstallReason::Explicit => "explicitly installed",
                    InstallReason::Dependency => "installed as dependency",
                }
            ));
        }
        Ok(out)
    }
}

#[derive(Debug)]
pub struct PackageRemove<'change> {
    pub remove: MaybeOwned<'change, [MaybeOwned<'change, str>]>,
}

impl<'change> Plannable<'change> for PackageRemove<'change> {
    fn plan<'elevate>(self: Box<Self>, elevate: &'elevate Elevate) -> Result<ChangeStatus<'change>>
    where
        'elevate: 'change,
    {
        let mut remove = Vec::new();

        for package in self.remove.as_ref() {
            if installed_packages(elevate)?
                .iter()
                .any(|s| s == package.as_ref())
            {
                remove.push(package.clone());
            }
        }

        if !remove.is_empty() {
            Ok(ChangeStatus::NeedsChange(Box::new(PackageRemovePlan {
                args: *self,
                remove,
            })))
        } else {
            Ok(ChangeStatus::Clean)
        }
    }
}

#[derive(Debug)]
pub(crate) struct PackageRemovePlan<'a> {
    // #[expect(unused, reason = "kept to have the same shape as other plans")]
    pub(crate) args: PackageRemove<'a>,
    pub(crate) remove: Vec<MaybeOwned<'a, str>>,
}

impl Applyable for PackageRemovePlan<'_> {
    fn apply(&self, elevate: &Elevate) -> Result<()> {
        if !self.remove.is_empty() {
            cmd::Process::run(
                "pacman",
                &vec!["--remove", "--noconfirm", "--"]
                    .tap_mut(|v| v.extend(self.remove.iter().map(|i| i.as_ref()))),
                elevate,
                None,
            )?
            .check()?;
        }

        Ok(())
    }
}

impl Reporter for PackageRemovePlan<'_> {
    fn report(&self) -> Result<Vec<String>> {
        let mut out = vec![];
        for install in &self.remove {
            out.push(format!("remove {name}", name = install.as_ref()));
        }
        Ok(out)
    }
}
