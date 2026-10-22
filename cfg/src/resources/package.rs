use anyhow::Result;
use nonempty::NonEmpty;
use tap::Tap as _;

use crate::maybeowned::MaybeOwned;

use super::super::{Applyable, ChangeStatus, Context, Plannable, Reporter, cmd};

#[derive(Debug)]
pub struct PackageDatabaseRefresh;

impl<'change> Plannable<'change> for PackageDatabaseRefresh {
    fn plan<'ctx>(self: Box<Self>, ctx: &'ctx Context<'ctx>) -> Result<ChangeStatus<'change>>
    where
        'ctx: 'change,
    {
        Ok(ChangeStatus::NeedsChange(Box::new(
            PackageDatabaseRefreshPlan { ctx, args: () },
        )))
    }
}

#[derive(Debug)]
pub(crate) struct PackageDatabaseRefreshPlan<'a> {
    pub(crate) ctx: &'a Context<'a>,
    // #[expect(unused, reason = "kept to have the same shape as other plans")]
    pub(crate) args: (),
}

impl Applyable for PackageDatabaseRefreshPlan<'_> {
    fn apply(&self) -> Result<()> {
        cmd::Process::run("pacman", &["--sync", "--refresh"], &self.ctx.elevate, None)?.check()?;
        Ok(())
    }
}

impl Reporter for PackageDatabaseRefreshPlan<'_> {
    fn report(&self) -> Result<Vec<String>> {
        Ok(vec!["package database refresh".to_owned()])
    }
}

#[derive(Debug)]
pub struct Packages<'change> {
    pub install: MaybeOwned<'change, [&'change str]>,
}

impl<'change> Plannable<'change> for Packages<'change> {
    fn plan<'ctx>(self: Box<Self>, ctx: &'ctx Context<'ctx>) -> Result<ChangeStatus<'change>>
    where
        'ctx: 'change,
    {
        let mut install = Vec::new();

        for package in self.install.as_ref() {
            if !ctx
                .initial_state
                .installed_packages
                .iter()
                .any(|s| s == package)
            {
                install.push(*package);
            }
        }

        if let Some(install) = NonEmpty::from_vec(install) {
            Ok(ChangeStatus::NeedsChange(Box::new(PackagePlan {
                ctx,
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
    pub(crate) ctx: &'a Context<'a>,
    // #[expect(unused, reason = "kept to have the same shape as other plans")]
    pub(crate) args: Packages<'a>,
    pub(crate) install: NonEmpty<&'a str>,
}

impl Applyable for PackagePlan<'_> {
    fn apply(&self) -> Result<()> {
        cmd::Process::run(
            "pacman",
            &vec!["--needed", "--noconfirm", "--sync", "--"].tap_mut(|v| v.extend(&self.install)),
            &self.ctx.elevate,
            None,
        )?
        .check()?;

        Ok(())
    }
}

impl Reporter for PackagePlan<'_> {
    fn report(&self) -> Result<Vec<String>> {
        Ok(self
            .install
            .iter()
            .map(|name| format!("install {name}"))
            .collect())
    }
}
