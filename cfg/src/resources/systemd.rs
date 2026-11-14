use anyhow::{Result, anyhow, bail};
use async_trait::async_trait;
use serde::Serialize;

use super::super::{Applyable, ChangeStatus, Elevate, Plannable, Reporter, cmd};

#[derive(Debug, PartialEq, Eq, Serialize, Clone, Copy)]
pub enum Target {
    MultiUser,
    Graphical,
}

impl Target {
    fn as_str(self) -> &'static str {
        match self {
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
pub enum ServiceStateRuntimeActual {
    Active,
    Inactive,
    ConditionNotMet,
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

#[derive(Debug)]
pub struct ServiceStateActual {
    pub runtime: ServiceStateRuntimeActual,
    pub persistent: ServiceStatePersistent,
}

impl ServiceState {
    fn diff(&self, other: &ServiceStateActual) -> ServiceStateDiff {
        ServiceStateDiff {
            runtime: match (other.runtime, self.runtime) {
                (ServiceStateRuntimeActual::Active, ServiceStateRuntime::Active)
                | (ServiceStateRuntimeActual::Inactive, ServiceStateRuntime::Inactive) => None,
                (ServiceStateRuntimeActual::Active, ServiceStateRuntime::Inactive) => {
                    Some(ServiceStateRuntime::Inactive)
                }
                (ServiceStateRuntimeActual::Inactive, ServiceStateRuntime::Active) => {
                    Some(ServiceStateRuntime::Active)
                }
                (ServiceStateRuntimeActual::ConditionNotMet, _) => {
                    // when condition not met, we never assume changes
                    None
                }
            },

            persistent: (self.persistent != other.persistent).then_some(self.persistent),
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

#[async_trait]
impl<'change> Plannable<'change> for Service<'change> {
    async fn plan<'elevate>(
        self: Box<Self>,
        elevate: &'elevate Elevate,
    ) -> Result<ChangeStatus<'change>>
    where
        'elevate: 'change,
    {
        let runtime_state = {
            // `ConditionResult` is weird. It may report `no` for stopped units
            // that never checked their condition (i.e. they were never started).
            // So we have to also check `ConditionTimestampMonotonic`, which is 0 when
            // the unit was never started. In that case, regardless of `ConditionResult`,
            // we report whatever `ActiveState` reports
            let systemctl = cmd::Process::run(
                "systemctl",
                &[
                    "show",
                    "-p",
                    "ActiveState",
                    "-p",
                    "ConditionTimestampMonotonic",
                    "-p",
                    "ConditionResult",
                    "--",
                    self.name,
                ],
                elevate,
                cmd::Stdin::Closed,
            )
            .await?
            .check()?;

            let lines: [&str; 3] = systemctl
                .stdout
                .trim()
                .lines()
                .collect::<Vec<&str>>()
                .try_into()
                .map_err(|_err| {
                    anyhow!("invalid systemctl show outputs lines for {}", self.name)
                })?;

            let Some(state) = lines
                .iter()
                .find_map(|line| line.strip_prefix("ActiveState="))
            else {
                bail!(
                    "invalid systemctl active output prefix for {} in \"{}\"",
                    self.name,
                    systemctl.stdout.trim()
                );
            };

            let Some(condition_checked) = lines
                .iter()
                .find_map(|line| line.strip_prefix("ConditionTimestampMonotonic="))
                .map(|s| s != "0")
            else {
                bail!(
                    "invalid systemctl condition timestamp prefix for {} in \"{}\"",
                    self.name,
                    systemctl.stdout.trim()
                );
            };

            let Some(condition_result) = lines
                .iter()
                .find_map(|line| line.strip_prefix("ConditionResult="))
                .map(|s| match s {
                    "yes" => Ok(true),
                    "no" => Ok(false),
                    other => Err(anyhow!(
                        "invalid conditionresult value for {}: {other}",
                        self.name
                    )),
                })
                .transpose()?
            else {
                bail!(
                    "invalid systemctl condition result output prefix for {} in \"{}\"",
                    self.name,
                    systemctl.stdout.trim()
                );
            };

            match (condition_checked, condition_result, state) {
                (true, false, _) => ServiceStateRuntimeActual::ConditionNotMet,
                (_, _, "active" | "activating" | "reloading" | "refreshing") => {
                    ServiceStateRuntimeActual::Active
                }
                (_, _, "inactive" | "failed" | "deactivating" | "maintenance") => {
                    ServiceStateRuntimeActual::Inactive
                }
                _ => bail!(
                    "invalid systemctl active output for {}: \"{}\"",
                    self.name,
                    state.trim()
                ),
            }
        };

        let persistent_state = {
            let systemctl = cmd::Process::run(
                "systemctl",
                &["is-enabled", "--", self.name],
                elevate,
                cmd::Stdin::Closed,
            )
            .await?
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

        let current_state = ServiceStateActual {
            runtime: runtime_state,
            persistent: persistent_state,
        };

        let diff = self.state.diff(&current_state);

        if diff.has_changes() {
            Ok(ChangeStatus::NeedsChange(Box::new(ServicePlan {
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
    pub(crate) args: Service<'a>,
    pub(crate) current_state: ServiceStateActual,
    pub(crate) diff: ServiceStateDiff,
}

#[async_trait]
impl Applyable for ServicePlan<'_> {
    async fn apply(&self, elevate: &Elevate) -> Result<()> {
        if let Some(persistent_state) = self.diff.persistent {
            match persistent_state {
                ServiceStatePersistent::Enabled => {
                    if self.current_state.persistent == ServiceStatePersistent::Masked {
                        cmd::Process::run(
                            "systemctl",
                            &["unmask", "--", self.args.name],
                            elevate,
                            cmd::Stdin::Closed,
                        )
                        .await?
                        .check()?;
                    }
                    cmd::Process::run(
                        "systemctl",
                        &["enable", "--", self.args.name],
                        elevate,
                        cmd::Stdin::Closed,
                    )
                    .await?
                    .check()?;
                }
                ServiceStatePersistent::Disabled => {
                    if self.current_state.persistent == ServiceStatePersistent::Masked {
                        cmd::Process::run(
                            "systemctl",
                            &["unmask", "--", self.args.name],
                            elevate,
                            cmd::Stdin::Closed,
                        )
                        .await?
                        .check()?;
                    }
                    cmd::Process::run(
                        "systemctl",
                        &["disable", "--", self.args.name],
                        elevate,
                        cmd::Stdin::Closed,
                    )
                    .await?
                    .check()?;
                }
                ServiceStatePersistent::Masked => {
                    cmd::Process::run(
                        "systemctl",
                        &["mask", "--", self.args.name],
                        elevate,
                        cmd::Stdin::Closed,
                    )
                    .await?
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
                        elevate,
                        cmd::Stdin::Closed,
                    )
                    .await?
                    .check()?;
                }
                ServiceStateRuntime::Inactive => {
                    cmd::Process::run(
                        "systemctl",
                        &["stop", "--", self.args.name],
                        elevate,
                        cmd::Stdin::Closed,
                    )
                    .await?
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

#[async_trait]
impl<'change> Plannable<'change> for RestartService<'change> {
    async fn plan<'elevate>(
        self: Box<Self>,
        _elevate: &'elevate Elevate,
    ) -> Result<ChangeStatus<'change>>
    where
        'elevate: 'change,
    {
        Ok(ChangeStatus::NeedsChange(Box::new(RestartServicePlan {
            args: *self,
        })))
    }
}

#[derive(Debug)]
pub(crate) struct RestartServicePlan<'a> {
    pub(crate) args: RestartService<'a>,
}

#[async_trait]
impl Applyable for RestartServicePlan<'_> {
    async fn apply(&self, elevate: &Elevate) -> Result<()> {
        cmd::Process::run(
            "systemctl",
            &["restart", "--", self.args.name],
            elevate,
            cmd::Stdin::Closed,
        )
        .await?
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

#[async_trait]
impl<'change> Plannable<'change> for DaemonReload {
    async fn plan<'elevate>(
        self: Box<Self>,
        _elevate: &'elevate Elevate,
    ) -> Result<ChangeStatus<'change>>
    where
        'elevate: 'change,
    {
        Ok(ChangeStatus::NeedsChange(Box::new(DaemonReloadPlan {
            args: *self,
        })))
    }
}

#[derive(Debug)]
pub(crate) struct DaemonReloadPlan {
    pub(crate) args: DaemonReload,
}

#[async_trait]
impl Applyable for DaemonReloadPlan {
    async fn apply(&self, elevate: &Elevate) -> Result<()> {
        cmd::Process::run("systemctl", &["daemon-reload"], elevate, cmd::Stdin::Closed)
            .await?
            .check()?;

        Ok(())
    }
}

impl Reporter for DaemonReloadPlan {
    fn report(&self) -> Result<Vec<String>> {
        Ok(vec!["systemd reloaded".to_owned()])
    }
}

#[derive(Debug)]
pub struct BootTarget {
    pub target: Target,
}

#[async_trait]
impl<'change> Plannable<'change> for BootTarget {
    async fn plan<'elevate>(
        self: Box<Self>,
        elevate: &'elevate Elevate,
    ) -> Result<ChangeStatus<'change>>
    where
        'elevate: 'change,
    {
        let current_target = {
            let systemctl =
                cmd::Process::run("systemctl", &["get-default"], elevate, cmd::Stdin::Closed)
                    .await?
                    .check()?;
            match systemctl.stdout.trim() {
                "multi-user.target" => Target::MultiUser,
                "graphical.target" => Target::Graphical,
                _ => bail!(
                    "invalid systemctl get-default output \"{}\"",
                    systemctl.stdout.trim()
                ),
            }
        };

        if current_target != self.target {
            Ok(ChangeStatus::NeedsChange(Box::new(BootTargetPlan {
                args: *self,
                current_target,
            })))
        } else {
            Ok(ChangeStatus::Clean)
        }
    }
}

#[derive(Debug)]
pub(crate) struct BootTargetPlan {
    pub(crate) args: BootTarget,
    pub(crate) current_target: Target,
}

#[async_trait]
impl Applyable for BootTargetPlan {
    async fn apply(&self, elevate: &Elevate) -> Result<()> {
        cmd::Process::run(
            "systemctl",
            &["set-default", "--", self.args.target.as_str()],
            elevate,
            cmd::Stdin::Closed,
        )
        .await?
        .check()?;

        Ok(())
    }
}

impl Reporter for BootTargetPlan {
    fn report(&self) -> Result<Vec<String>> {
        Ok(vec![format!(
            "default boot target: {} -> {}",
            self.current_target.as_str(),
            self.args.target.as_str()
        )])
    }
}
