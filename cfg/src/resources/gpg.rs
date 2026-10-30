use anyhow::Result;

use crate::maybeowned::MaybeOwned;

use super::super::{Applyable, ChangeStatus, Context, Path, Plannable, Reporter, gpg};

#[derive(Debug)]
pub struct GpgKey<'a> {
    pub content: &'a [u8],
    pub trust: gpg::GpgTrust,
    pub fingerprint: gpg::GpgFingerprint,
    pub gnupg_directory: MaybeOwned<'a, Path>,
}

impl<'change> Plannable<'change> for GpgKey<'change> {
    fn plan<'ctx>(self: Box<Self>, ctx: &'ctx Context<'ctx>) -> Result<ChangeStatus<'change>>
    where
        'ctx: 'change,
    {
        let key = gpg::get_gpg_key_trust(
            &self.fingerprint,
            self.gnupg_directory.as_ref(),
            &ctx.elevate,
        )?;
        match key {
            Some(trust) => {
                if trust == self.trust {
                    Ok(ChangeStatus::Clean)
                } else {
                    Ok(ChangeStatus::NeedsChange(Box::new(GpgPlan {
                        ctx,
                        args: *self,
                        action: GpgAction::Trust {
                            existing_trust: trust,
                        },
                    })))
                }
            }
            None => Ok(ChangeStatus::NeedsChange(Box::new(GpgPlan {
                ctx,
                args: *self,
                action: GpgAction::Import,
            }))),
        }
    }
}

#[derive(Debug)]
enum GpgAction {
    Import,
    Trust { existing_trust: gpg::GpgTrust },
}

#[derive(Debug)]
pub(crate) struct GpgPlan<'a> {
    ctx: &'a Context<'a>,
    args: GpgKey<'a>,
    action: GpgAction,
}

impl Applyable for GpgPlan<'_> {
    fn apply(&self) -> Result<()> {
        match self.action {
            GpgAction::Import => {
                gpg::import_key(
                    self.args.content,
                    self.args.gnupg_directory.as_ref(),
                    &self.ctx.elevate,
                )?;
                gpg::trust_key(
                    &self.args.fingerprint,
                    self.args.trust,
                    self.args.gnupg_directory.as_ref(),
                    &self.ctx.elevate,
                )?;
            }
            GpgAction::Trust { existing_trust: _ } => {
                gpg::trust_key(
                    &self.args.fingerprint,
                    self.args.trust,
                    self.args.gnupg_directory.as_ref(),
                    &self.ctx.elevate,
                )?;
            }
        }
        Ok(())
    }
}

impl Reporter for GpgPlan<'_> {
    fn report(&self) -> Result<Vec<String>> {
        let mut out = vec![];

        match self.action {
            GpgAction::Import => out.push("importing key".to_owned()),
            GpgAction::Trust { ref existing_trust } => out.push(format!(
                "trusing existing key {} (currently {existing_trust})",
                self.args.trust,
            )),
        }
        // out.push(format!("persistent: {} -> {}",));

        Ok(out)
    }
}
