use anyhow::Result;

use super::super::{Applyable, ChangeStatus, Context, Plannable, Reporter, cmd};

#[derive(Debug, Clone)]
pub struct Command<'a> {
    pub cmd: &'a str,
    pub args: &'a [&'a str],
}

impl<'change> Plannable<'change> for Command<'change> {
    fn plan<'ctx>(self: Box<Self>, ctx: &'ctx Context<'ctx>) -> Result<ChangeStatus<'change>>
    where
        'ctx: 'change,
    {
        Ok(ChangeStatus::NeedsChange(Box::new(CommandPlan {
            ctx,
            args: *self,
        })))
    }
}

#[derive(Debug)]
pub(crate) struct CommandPlan<'a> {
    pub(crate) ctx: &'a Context<'a>,
    pub(crate) args: Command<'a>,
}

impl Applyable for CommandPlan<'_> {
    fn apply(&self) -> Result<()> {
        cmd::Process::run(self.args.cmd, self.args.args, &self.ctx.elevate, None)?.check()?;

        Ok(())
    }
}

impl Reporter for CommandPlan<'_> {
    fn report(&self) -> Result<Vec<String>> {
        Ok(vec![
            (format!(
                "executed {} {}",
                self.args.cmd,
                self.args.args.to_vec().join(" ")
            )),
        ])
    }
}
