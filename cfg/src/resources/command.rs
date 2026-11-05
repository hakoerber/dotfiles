use anyhow::{Result, bail};

use super::super::{Applyable, ChangeStatus, Elevate, MaybeOwned, Plannable, Reporter, cmd};

#[derive(Debug, Clone)]
pub struct CheckScript<'a> {
    pub content: MaybeOwned<'a, [u8]>,
}

#[derive(Debug, Clone)]
pub struct Command<'a> {
    pub cmd: MaybeOwned<'a, str>,
    pub args: &'a [&'a str],
    pub check_command: Option<CheckScript<'a>>,
}

#[async_trait::async_trait]
impl<'change> Plannable<'change> for Command<'change> {
    async fn plan<'elevate>(
        self: Box<Self>,
        elevate: &'elevate Elevate,
    ) -> Result<ChangeStatus<'change>>
    where
        'elevate: 'change,
    {
        let needs_execution = if let Some(ref script) = self.check_command {
            let command = cmd::Process::run("sh", &[], elevate, Some(script.content.as_ref()))
                .await?
                .unchecked();
            match command.status.code() {
                0 => false,
                123 => true,
                other => bail!(
                    "invalid exit code of check script: {other} {:?} {:?}",
                    command.stdout,
                    command.stderr
                ),
            }
        } else {
            true
        };

        if needs_execution {
            Ok(ChangeStatus::NeedsChange(Box::new(CommandPlan {
                args: *self,
            })))
        } else {
            Ok(ChangeStatus::Clean)
        }
    }
}

#[derive(Debug)]
pub(crate) struct CommandPlan<'a> {
    pub(crate) args: Command<'a>,
}

#[async_trait::async_trait]
impl Applyable for CommandPlan<'_> {
    async fn apply(&self, elevate: &Elevate) -> Result<()> {
        cmd::Process::run(self.args.cmd.as_ref(), self.args.args, elevate, None)
            .await?
            .check()?;

        Ok(())
    }
}

impl Reporter for CommandPlan<'_> {
    fn report(&self) -> Result<Vec<String>> {
        Ok(vec![
            (format!(
                "executed {} {}",
                self.args.cmd.as_ref(),
                self.args.args.to_vec().join(" ")
            )),
        ])
    }
}
