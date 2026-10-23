use anyhow::Result;
use nonempty::NonEmpty;
use tap::Tap as _;

use super::{Applyable, ChangeStatus, Context, Plannable, Reporter, cmd};

#[derive(Debug)]
pub(crate) struct Packages<'a> {
    pub(crate) install: &'a [&'a str],
}

impl<'a> Plannable<'a> for Packages<'a> {
    fn plan(self: Box<Self>, ctx: &'a Context<'a>) -> Result<ChangeStatus<'a>> {
        let mut install = Vec::new();
        for package in self.install {
            if cmd::cmd(
                &vec!["pacman", "--query", "--quiet", "--", package],
                &ctx.elevate,
            )?
            .status
            .code()
                == 1
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
    pub(crate) args: Packages<'a>,
    pub(crate) install: NonEmpty<&'a str>,
}

impl Applyable for PackagePlan<'_> {
    fn apply(&self) -> Result<()> {
        cmd::cmd(
            &vec!["pacman", "--needed", "--noconfirm", "--sync", "--"]
                .tap_mut(|v| v.extend(&self.install)),
            &self.ctx.elevate,
        )?;

        Ok(())
    }
}

impl Reporter for PackagePlan<'_> {
    fn report(&self) -> Vec<String> {
        self.install
            .iter()
            .map(|name| format!("install {name}"))
            .collect()
    }
}
