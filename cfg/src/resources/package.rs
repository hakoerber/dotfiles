use anyhow::Result;
use async_once_cell::OnceCell;
use tap::Tap as _;

use super::super::{
    Applyable, ChangeStatus, Elevate, Path, Plannable, Reporter, cmd,
    maybeowned::MaybeOwned,
    pkg::{self, InstallReason},
};

static INSTALLED_PACKAGES: OnceCell<Vec<String>> = OnceCell::new();

async fn installed_packages(elevate: &Elevate) -> Result<&Vec<String>> {
    let packages = INSTALLED_PACKAGES
        .get_or_try_init(async { pkg::list_installed_packages(elevate).await })
        .await?;
    Ok(packages)
}

static EXPLICITLY_INSTALLED_PACKAGES: OnceCell<Vec<String>> = OnceCell::new();

async fn explicitly_installed_packages(elevate: &Elevate) -> Result<&Vec<String>> {
    let packages = EXPLICITLY_INSTALLED_PACKAGES
        .get_or_try_init(async { pkg::list_explicitly_installed_packages(elevate).await })
        .await?;
    Ok(packages)
}

static PACKAGES_INSTALLED_AS_DEPENDENCIES: OnceCell<Vec<String>> = OnceCell::new();

async fn packages_installed_as_dependencies(elevate: &Elevate) -> Result<&Vec<String>> {
    let packages = PACKAGES_INSTALLED_AS_DEPENDENCIES
        .get_or_try_init(async { pkg::list_packages_installed_as_dependencies(elevate).await })
        .await?;
    Ok(packages)
}

#[derive(Debug)]
pub struct PackageDatabaseRefresh;

#[async_trait::async_trait]
impl<'change> Plannable<'change> for PackageDatabaseRefresh {
    async fn plan<'elevate>(
        self: Box<Self>,
        _elevate: &'elevate Elevate,
    ) -> Result<ChangeStatus<'change>>
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

#[async_trait::async_trait]
impl Applyable for PackageDatabaseRefreshPlan<'_> {
    async fn apply(&self, elevate: &Elevate) -> Result<()> {
        cmd::Process::run("pacman", &["--sync", "--refresh"], elevate, None)
            .await?
            .check()?;
        Ok(())
    }
}

impl Reporter for PackageDatabaseRefreshPlan<'_> {
    fn report(&self) -> Result<Vec<String>> {
        Ok(vec!["package database refresh".to_owned()])
    }
}

#[derive(Debug)]
pub struct Packages<'action> {
    pub install: MaybeOwned<'action, [MaybeOwned<'action, str>]>,
    pub reason: InstallReason,
}

#[async_trait::async_trait]
impl<'change> Plannable<'change> for Packages<'change> {
    async fn plan<'elevate>(
        self: Box<Self>,
        elevate: &'elevate Elevate,
    ) -> Result<ChangeStatus<'change>>
    where
        'elevate: 'change,
    {
        let mut install = Vec::new();

        for package in self.install.as_ref() {
            if !installed_packages(elevate)
                .await?
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
                    if !explicitly_installed_packages(elevate)
                        .await?
                        .iter()
                        .any(|s| s == package.as_ref())
                    {
                        set_reason.push(package.clone());
                    }
                }
            }
            InstallReason::Dependency => {
                for package in self.install.as_ref() {
                    if !packages_installed_as_dependencies(elevate)
                        .await?
                        .iter()
                        .any(|s| s == package.as_ref())
                    {
                        set_reason.push(package.clone());
                    }
                }
            }
            InstallReason::DependencyIfInstalling => {
                // in this case, we retain the current setting, except when we install new
            }
        }

        if !install.is_empty() || !set_reason.is_empty() {
            Ok(ChangeStatus::NeedsChange(Box::new(PackagesPlan {
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
pub(crate) struct PackagesPlan<'a> {
    // #[expect(unused, reason = "kept to have the same shape as other plans")]
    pub(crate) args: Packages<'a>,
    pub(crate) install: Vec<MaybeOwned<'a, str>>,
    pub(crate) set_reason: Vec<MaybeOwned<'a, str>>,
}

#[async_trait::async_trait]
impl Applyable for PackagesPlan<'_> {
    async fn apply(&self, elevate: &Elevate) -> Result<()> {
        if !self.install.is_empty() {
            cmd::Process::run(
                "pacman",
                &vec!["--needed", "--noconfirm", "--sync", "--"]
                    .tap_mut(|v| v.extend(self.install.iter().map(|i| i.as_ref()))),
                elevate,
                None,
            )
            .await?
            .check()?;
        }

        if !self.set_reason.is_empty() {
            cmd::Process::run(
                "pacman",
                &vec![
                    "--database",
                    match self.args.reason {
                        InstallReason::Explicit => "--asexplicit",
                        InstallReason::Dependency | InstallReason::DependencyIfInstalling => {
                            "--asdeps"
                        }
                    },
                    "--",
                ]
                .tap_mut(|v| v.extend(self.set_reason.iter().map(|i| i.as_ref()))),
                elevate,
                None,
            )
            .await?
            .check()?;
        }

        Ok(())
    }
}

impl Reporter for PackagesPlan<'_> {
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
                    InstallReason::Dependency | InstallReason::DependencyIfInstalling =>
                        "installed as dependency",
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

#[async_trait::async_trait]
impl<'change> Plannable<'change> for PackageRemove<'change> {
    async fn plan<'elevate>(
        self: Box<Self>,
        elevate: &'elevate Elevate,
    ) -> Result<ChangeStatus<'change>>
    where
        'elevate: 'change,
    {
        let mut remove = Vec::new();

        for package in self.remove.as_ref() {
            if installed_packages(elevate)
                .await?
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

#[async_trait::async_trait]
impl Applyable for PackageRemovePlan<'_> {
    async fn apply(&self, elevate: &Elevate) -> Result<()> {
        if !self.remove.is_empty() {
            cmd::Process::run(
                "pacman",
                &vec!["--remove", "--noconfirm", "--"]
                    .tap_mut(|v| v.extend(self.remove.iter().map(|i| i.as_ref()))),
                elevate,
                None,
            )
            .await?
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

#[derive(Debug)]
pub struct LocalPackage<'action> {
    pub name: MaybeOwned<'action, str>,
    pub path: MaybeOwned<'action, Path>,
    pub version: MaybeOwned<'action, str>,
    pub reason: InstallReason,
}

#[async_trait::async_trait]
impl<'change> Plannable<'change> for LocalPackage<'change> {
    async fn plan<'elevate>(
        self: Box<Self>,
        elevate: &'elevate Elevate,
    ) -> Result<ChangeStatus<'change>>
    where
        'elevate: 'change,
    {
        let package_version = pkg::get_package_version(self.name.as_ref(), elevate).await?;

        match package_version {
            Some(installed_version) => {
                let set_reason = match self.reason {
                    InstallReason::Explicit => !explicitly_installed_packages(elevate)
                        .await?
                        .iter()
                        .any(|s| s == self.name.as_ref()),
                    InstallReason::Dependency => !packages_installed_as_dependencies(elevate)
                        .await?
                        .iter()
                        .any(|s| s == self.name.as_ref()),
                    InstallReason::DependencyIfInstalling => false,
                };

                if installed_version != self.version.as_ref() {
                    Ok(ChangeStatus::NeedsChange(Box::new(LocalPackagePlan {
                        change: LocalPackageChange {
                            operation: Some(LocalPackageChangeOperation::ChangeVersion {
                                to_version: self.version.clone(),
                            }),
                            set_reason,
                        },
                        args: *self,
                    })))
                } else if set_reason {
                    Ok(ChangeStatus::NeedsChange(Box::new(LocalPackagePlan {
                        args: *self,
                        change: LocalPackageChange {
                            operation: None,
                            set_reason: true,
                        },
                    })))
                } else {
                    Ok(ChangeStatus::Clean)
                }
            }
            None => Ok(ChangeStatus::NeedsChange(Box::new(LocalPackagePlan {
                args: *self,
                change: LocalPackageChange {
                    operation: Some(LocalPackageChangeOperation::Install),
                    set_reason: false,
                },
            }))),
        }
    }
}

#[derive(Debug)]
pub(crate) enum LocalPackageChangeOperation<'a> {
    Install,
    ChangeVersion { to_version: MaybeOwned<'a, str> },
}

#[derive(Debug)]
pub(crate) struct LocalPackageChange<'a> {
    operation: Option<LocalPackageChangeOperation<'a>>,
    set_reason: bool,
}

#[derive(Debug)]
pub(crate) struct LocalPackagePlan<'a> {
    // #[expect(unused, reason = "kept to have the same shape as other plans")]
    pub(crate) args: LocalPackage<'a>,
    pub(crate) change: LocalPackageChange<'a>,
}

#[async_trait::async_trait]
impl Applyable for LocalPackagePlan<'_> {
    async fn apply(&self, elevate: &Elevate) -> Result<()> {
        match self.change.operation {
            None => (),
            Some(
                LocalPackageChangeOperation::Install
                | LocalPackageChangeOperation::ChangeVersion { to_version: _ },
            ) => {
                cmd::Process::run(
                    "pacman",
                    &[
                        "--needed",
                        "--noconfirm",
                        "--upgrade",
                        "--",
                        self.args.path.as_ref().as_str(),
                    ],
                    elevate,
                    None,
                )
                .await?
                .check()?;
            }
        }

        if self.change.set_reason {
            cmd::Process::run(
                "pacman",
                &[
                    "--database",
                    match self.args.reason {
                        InstallReason::Explicit => "--asexplicit",
                        InstallReason::Dependency => "--asdeps",
                        InstallReason::DependencyIfInstalling => unreachable!(),
                    },
                    "--",
                    self.args.name.as_ref(),
                ],
                elevate,
                None,
            )
            .await?
            .check()?;
        }

        Ok(())
    }
}

impl Reporter for LocalPackagePlan<'_> {
    fn report(&self) -> Result<Vec<String>> {
        let mut out = vec![];

        match self.change.operation {
            Some(LocalPackageChangeOperation::Install) => {
                out.push(format!("install {name}", name = self.args.name.as_ref()));
            }
            Some(LocalPackageChangeOperation::ChangeVersion { ref to_version }) => {
                out.push(format!(
                    "change version of {name} to {version}",
                    version = to_version.as_ref(),
                    name = self.args.name.as_ref()
                ));
            }
            None => (),
        }

        if self.change.set_reason {
            out.push(format!(
                "set {} as {}",
                self.args.name.as_ref(),
                match self.args.reason {
                    InstallReason::Explicit => "explicitly installed",
                    InstallReason::Dependency => "installed as dependency",
                    InstallReason::DependencyIfInstalling => unreachable!(),
                }
            ));
        }

        Ok(out)
    }
}

#[derive(Debug)]
pub struct SinglePackage<'action> {
    pub package: MaybeOwned<'action, str>,
    pub reason: InstallReason,
}

#[async_trait::async_trait]
impl<'change> Plannable<'change> for SinglePackage<'change> {
    async fn plan<'elevate>(
        self: Box<Self>,
        elevate: &'elevate Elevate,
    ) -> Result<ChangeStatus<'change>>
    where
        'elevate: 'change,
    {
        if !installed_packages(elevate)
            .await?
            .iter()
            .any(|s| s == self.package.as_ref())
        {
            Ok(ChangeStatus::NeedsChange(Box::new(SinglePackagePlan {
                args: *self,
                action: SinglePackagePlanAction::Install,
            })))
        } else {
            match self.reason {
                InstallReason::Explicit => {
                    if !explicitly_installed_packages(elevate)
                        .await?
                        .iter()
                        .any(|s| s == self.package.as_ref())
                    {
                        Ok(ChangeStatus::NeedsChange(Box::new(SinglePackagePlan {
                            args: *self,
                            action: SinglePackagePlanAction::SetReason,
                        })))
                    } else {
                        Ok(ChangeStatus::Clean)
                    }
                }
                InstallReason::Dependency => {
                    if !packages_installed_as_dependencies(elevate)
                        .await?
                        .iter()
                        .any(|s| s == self.package.as_ref())
                    {
                        Ok(ChangeStatus::NeedsChange(Box::new(SinglePackagePlan {
                            args: *self,
                            action: SinglePackagePlanAction::SetReason,
                        })))
                    } else {
                        Ok(ChangeStatus::Clean)
                    }
                }
                InstallReason::DependencyIfInstalling => {
                    // in this case, we retain the current setting
                    Ok(ChangeStatus::Clean)
                }
            }
        }
    }
}

#[derive(Debug)]
pub(crate) enum SinglePackagePlanAction {
    Install,
    SetReason,
}

#[derive(Debug)]
pub(crate) struct SinglePackagePlan<'a> {
    // #[expect(unused, reason = "kept to have the same shape as other plans")]
    pub(crate) args: SinglePackage<'a>,
    pub(crate) action: SinglePackagePlanAction,
}

#[async_trait::async_trait]
impl Applyable for SinglePackagePlan<'_> {
    async fn apply(&self, elevate: &Elevate) -> Result<()> {
        // Resolve the package before installing. another package that was installed in the
        // meantime may "provide" it. That cannot be seen during planning.
        let resolved = pkg::resolve_installed_package(elevate, self.args.package.as_ref()).await?;

        let name = resolved
            .as_deref()
            .unwrap_or_else(|| self.args.package.as_ref());

        match self.action {
            SinglePackagePlanAction::Install => {
                cmd::Process::run(
                    "pacman",
                    &["--needed", "--noconfirm", "--sync", "--", name],
                    elevate,
                    None,
                )
                .await?
                .check()?;

                // Explicitly set the reason again, as the resolved package have been installed as a
                // dependency in the mean time. Note that this can only "upgrade" the reason (from
                // `Dependency` to `Explicit`). Never downgrade.
                let reason = pkg::get_install_reason(elevate, name)
                    .await?
                    .expect("installed package has an install reason");

                if reason == InstallReason::Dependency
                    && self.args.reason == InstallReason::Explicit
                {
                    cmd::Process::run(
                        "pacman",
                        &["--database", "--asexplicit", "--", name],
                        elevate,
                        None,
                    )
                    .await?
                    .check()?;
                }
            }
            SinglePackagePlanAction::SetReason => {
                cmd::Process::run(
                    "pacman",
                    &[
                        "--database",
                        match self.args.reason {
                            InstallReason::Explicit => "--asexplicit",
                            InstallReason::Dependency | InstallReason::DependencyIfInstalling => {
                                "--asdeps"
                            }
                        },
                        "--",
                        name,
                    ],
                    elevate,
                    None,
                )
                .await?
                .check()?;
            }
        }

        Ok(())
    }
}

impl Reporter for SinglePackagePlan<'_> {
    fn report(&self) -> Result<Vec<String>> {
        match self.action {
            SinglePackagePlanAction::Install => Ok(vec![format!(
                "install {name}",
                name = self.args.package.as_ref()
            )]),
            SinglePackagePlanAction::SetReason => Ok(vec![format!(
                "set {name} as {message}",
                name = self.args.package.as_ref(),
                message = match self.args.reason {
                    InstallReason::Explicit => "explicitly installed",
                    InstallReason::Dependency | InstallReason::DependencyIfInstalling =>
                        "installed as dependency",
                }
            )]),
        }
    }
}
