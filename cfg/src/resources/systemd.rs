use anyhow::{Result, bail};
use serde::Serialize;

use super::super::{Applyable, ChangeStatus, Context, Plannable, Reporter, cmd};

#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum Target {
    MultiUser,
    Graphical,
}

impl Target {
    fn as_str(&self) -> &'static str {
        match *self {
            Self::MultiUser => "multi-user.target",
            Self::Graphical => "graphical.target",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, derive_more::Display)]
pub enum ServiceStateRuntime {
    Active,
    Inactive,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, derive_more::Display)]
pub enum ServiceStatePersistent {
    Enabled,
    Disabled,
    Masked,
}

#[derive(Debug)]
pub struct ServiceState {
    pub runtime: ServiceStateRuntime,
    pub persistent: ServiceStatePersistent,
}

impl ServiceState {
    fn diff(&self, other: &Self) -> ServiceStateDiff {
        ServiceStateDiff {
            runtime: (self.runtime != other.runtime).then_some(other.runtime),
            persistent: (self.persistent != other.persistent).then_some(other.persistent),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ServiceStateDiff {
    runtime: Option<ServiceStateRuntime>,
    persistent: Option<ServiceStatePersistent>,
}

impl ServiceStateDiff {
    fn is_empty(&self) -> bool {
        self.runtime.is_none() && self.persistent.is_none()
    }

    fn has_changes(&self) -> bool {
        !self.is_empty()
    }
}

#[derive(Debug)]
pub struct Service<'a> {
    pub name: &'a str,
    pub state: ServiceState,
}

impl<'change> Plannable<'change> for Service<'change> {
    fn plan<'ctx>(self: Box<Self>, ctx: &'ctx Context<'ctx>) -> Result<ChangeStatus<'change>>
    where
        'ctx: 'change,
    {
        let runtime_state = {
            let systemctl = cmd::Process::run(
                "systemctl",
                &["show", "-p", "ActiveState", "--", self.name],
                &ctx.elevate,
                None,
            )?
            .check()?;
            let Some(state) = systemctl.stdout.trim().strip_prefix("ActiveState=") else {
                bail!(
                    "invalid systemctl active output prefix in \"{}\"",
                    systemctl.stdout.trim()
                );
            };

            match state {
                "active" | "activating" | "reloading" | "refreshing" => ServiceStateRuntime::Active,
                "inactive" | "failed" | "deactivating" | "maintenance" => {
                    ServiceStateRuntime::Inactive
                }
                _ => bail!("invalid systemctl active output \"{}\"", state.trim()),
            }
        };

        let persistent_state = {
            let systemctl = cmd::Process::run(
                "systemctl",
                &["is-enabled", "--", self.name],
                &ctx.elevate,
                None,
            )?
            .unchecked();

            match systemctl.stdout.trim() {
                "enabled" => ServiceStatePersistent::Enabled,
                // we treat not-found as disabled as well. the unit may not exist (yet)
                // it's the same for ActiveState above, which also reports "inactive" for
                // non-existent units
                "disabled" | "not-found" => ServiceStatePersistent::Disabled,
                "masked" => ServiceStatePersistent::Masked,
                _ => bail!(
                    "invalid systemctl is-enabled output \"{}\"",
                    systemctl.stdout.trim()
                ),
            }
        };

        let current_state = ServiceState {
            runtime: runtime_state,
            persistent: persistent_state,
        };

        let diff = current_state.diff(&self.state);

        if diff.has_changes() {
            Ok(ChangeStatus::NeedsChange(Box::new(ServicePlan {
                ctx,
                args: *self,
                current_state,
                diff,
            })))
        } else {
            Ok(ChangeStatus::Clean)
        }
    }
}

#[derive(Debug)]
pub(crate) struct ServicePlan<'a> {
    pub(crate) ctx: &'a Context<'a>,
    pub(crate) args: Service<'a>,
    pub(crate) current_state: ServiceState,
    pub(crate) diff: ServiceStateDiff,
}

impl Applyable for ServicePlan<'_> {
    fn apply(&self) -> Result<()> {
        if let Some(persistent_state) = self.diff.persistent {
            match persistent_state {
                ServiceStatePersistent::Enabled => {
                    if self.current_state.persistent == ServiceStatePersistent::Masked {
                        cmd::Process::run(
                            "systemctl",
                            &["unmask", "--", self.args.name],
                            &self.ctx.elevate,
                            None,
                        )?
                        .check()?;
                    }
                    cmd::Process::run(
                        "systemctl",
                        &["enable", "--", self.args.name],
                        &self.ctx.elevate,
                        None,
                    )?
                    .check()?;
                }
                ServiceStatePersistent::Disabled => {
                    if self.current_state.persistent == ServiceStatePersistent::Masked {
                        cmd::Process::run(
                            "systemctl",
                            &["unmask", "--", self.args.name],
                            &self.ctx.elevate,
                            None,
                        )?
                        .check()?;
                    }
                    cmd::Process::run(
                        "systemctl",
                        &["disable", "--", self.args.name],
                        &self.ctx.elevate,
                        None,
                    )?
                    .check()?;
                }
                ServiceStatePersistent::Masked => {
                    cmd::Process::run(
                        "systemctl",
                        &["mask", "--", self.args.name],
                        &self.ctx.elevate,
                        None,
                    )?
                    .check()?;
                }
            }
        }

        // do the runtime state after the persistent state, otherwise the unit may be masked
        if let Some(runtime_state) = self.diff.runtime {
            match runtime_state {
                ServiceStateRuntime::Active => {
                    cmd::Process::run(
                        "systemctl",
                        &["start", "--", self.args.name],
                        &self.ctx.elevate,
                        None,
                    )?
                    .check()?;
                }
                ServiceStateRuntime::Inactive => {
                    cmd::Process::run(
                        "systemctl",
                        &["stop", "--", self.args.name],
                        &self.ctx.elevate,
                        None,
                    )?
                    .check()?;
                }
            }
        }

        Ok(())
    }
}

impl Reporter for ServicePlan<'_> {
    fn report(&self) -> Result<Vec<String>> {
        let mut out = vec![];

        if let Some(runtime_state) = self.diff.runtime {
            out.push(format!(
                "runtime state: {} -> {}",
                self.current_state.runtime, runtime_state
            ));
        }

        if let Some(persistent_state) = self.diff.persistent {
            out.push(format!(
                "persistent: {} -> {}",
                self.current_state.persistent, persistent_state
            ));
        }

        Ok(out)
    }
}

#[derive(Debug)]
pub struct RestartService<'a> {
    pub name: &'a str,
}

impl<'change> Plannable<'change> for RestartService<'change> {
    fn plan<'ctx>(self: Box<Self>, ctx: &'ctx Context<'ctx>) -> Result<ChangeStatus<'change>>
    where
        'ctx: 'change,
    {
        Ok(ChangeStatus::NeedsChange(Box::new(RestartServicePlan {
            ctx,
            args: *self,
        })))
    }
}

#[derive(Debug)]
pub(crate) struct RestartServicePlan<'a> {
    pub(crate) ctx: &'a Context<'a>,
    pub(crate) args: RestartService<'a>,
}

impl Applyable for RestartServicePlan<'_> {
    fn apply(&self) -> Result<()> {
        cmd::Process::run(
            "systemctl",
            &["restart", "--", self.args.name],
            &self.ctx.elevate,
            None,
        )?
        .check()?;

        Ok(())
    }
}

impl Reporter for RestartServicePlan<'_> {
    fn report(&self) -> Result<Vec<String>> {
        Ok(vec![(format!("{} restarted", self.args.name))])
    }
}

#[derive(Debug)]
pub struct DaemonReload;

impl<'change> Plannable<'change> for DaemonReload {
    fn plan<'ctx>(self: Box<Self>, ctx: &'ctx Context<'ctx>) -> Result<ChangeStatus<'change>>
    where
        'ctx: 'change,
    {
        Ok(ChangeStatus::NeedsChange(Box::new(DaemonReloadPlan {
            ctx,
            args: *self,
        })))
    }
}

#[derive(Debug)]
pub(crate) struct DaemonReloadPlan<'a> {
    pub(crate) ctx: &'a Context<'a>,
    pub(crate) args: DaemonReload,
}

impl Applyable for DaemonReloadPlan<'_> {
    fn apply(&self) -> Result<()> {
        cmd::Process::run("systemctl", &["daemon-reload"], &self.ctx.elevate, None)?.check()?;

        Ok(())
    }
}

impl Reporter for DaemonReloadPlan<'_> {
    fn report(&self) -> Result<Vec<String>> {
        Ok(vec!["systemd reloaded".to_owned()])
    }
}

#[derive(Debug)]
pub struct BootTarget<'a> {
    pub target: &'a Target,
}

impl<'change> Plannable<'change> for BootTarget<'change> {
    fn plan<'ctx>(self: Box<Self>, ctx: &'ctx Context<'ctx>) -> Result<ChangeStatus<'change>>
    where
        'ctx: 'change,
    {
        let current_target = {
            let systemctl =
                cmd::Process::run("systemctl", &["get-default"], &ctx.elevate, None)?.check()?;
            match systemctl.stdout.trim() {
                "multi-user.target" => Target::MultiUser,
                "graphical.target" => Target::Graphical,
                _ => bail!(
                    "invalid systemctl get-default output \"{}\"",
                    systemctl.stdout.trim()
                ),
            }
        };

        if current_target != *self.target {
            Ok(ChangeStatus::NeedsChange(Box::new(BootTargetPlan {
                ctx,
                args: *self,
                current_target,
            })))
        } else {
            Ok(ChangeStatus::Clean)
        }
    }
}

#[derive(Debug)]
pub(crate) struct BootTargetPlan<'a> {
    pub(crate) ctx: &'a Context<'a>,
    pub(crate) args: BootTarget<'a>,
    pub(crate) current_target: Target,
}

impl Applyable for BootTargetPlan<'_> {
    fn apply(&self) -> Result<()> {
        cmd::Process::run(
            "systemctl",
            &["set-default", "--", self.args.target.as_str()],
            &self.ctx.elevate,
            None,
        )?
        .check()?;

        Ok(())
    }
}

impl Reporter for BootTargetPlan<'_> {
    fn report(&self) -> Result<Vec<String>> {
        Ok(vec![format!(
            "default boot target: {} -> {}",
            self.current_target.as_str(),
            self.args.target.as_str()
        )])
    }
}
