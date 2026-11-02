#![allow(unused, reason = "dev")]
#![deny(unused_must_use)]
#![feature(once_cell_try)]

use std::{
    fmt,
    io::{self, Write},
    mem,
};

use anyhow::Result;

pub use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};

mod checksum;
mod cmd;
mod fs;
mod gpg;
mod maybeowned;
mod pkg;
mod resources;
mod tempfile;
mod user;

use checksum::Checksum;
use maybeowned::MaybeOwned;
use owo_colors::OwoColorize;
use owo_colors::colors;
use tempfile::Tempfile;

pub mod prelude {
    pub use super::cmd::Process;
    pub use super::fs::{
        FileContent::{self, Lines},
        FileMode, FilePermissions,
    };
    pub use super::gpg::{GpgFingerprint, GpgTrust};
    pub use super::maybeowned::MaybeOwned::{self, Owned, Ref};
    pub use super::pkg::{
        check_installed, get_package_version, list_explicitly_installed_packages,
        list_orphaned_packages,
    };
    pub use super::resources::prelude::*;
    pub use super::{Elevate, ElevateTo, Gid, Path, PathBuf, Plan, ReportConfig, Resource, Uid};
}

#[derive(Eq, PartialEq, Debug)]
pub enum ElevateTo {
    Root,
    User { user: String },
}

#[derive(Eq, PartialEq, Debug)]
pub enum Elevate {
    Sudo(ElevateTo),
    No,
}

pub trait Reporter {
    fn report(&self) -> Result<Vec<String>>;
}

#[derive(Debug)]
pub enum ChangeStatus<'action> {
    NeedsChange(Box<dyn Applyable + 'action>),
    Clean,
}

impl ChangeStatus<'_> {
    fn changed(&self) -> bool {
        matches!(self, Self::NeedsChange(_))
    }
}

enum Trigger<'name, 'elevate, 'action> {
    OnChange(Box<dyn FnOnce() -> Result<Resource<'name, 'elevate, 'action>> + 'elevate>),
}

impl fmt::Debug for Trigger<'_, '_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::OnChange(ref arg0) => f.debug_tuple("OnChange").field(&"[fun]").finish(),
        }
    }
}

#[derive(Debug)]
pub struct Resource<'name, 'elevate, 'action> {
    name: MaybeOwned<'name, str>,
    elevate: &'elevate Elevate,
    action: Box<dyn Plannable<'action> + 'action>,
    triggers: Vec<Trigger<'name, 'elevate, 'action>>,
}

impl<'name, 'elevate: 'action, 'action> Resource<'name, 'elevate, 'action> {
    pub fn new(
        name: MaybeOwned<'name, str>,
        elevate: &'elevate Elevate,
        action: Box<dyn Plannable<'action> + 'action>,
    ) -> Self {
        Self {
            name,
            elevate,
            action,
            triggers: vec![],
        }
    }

    fn plan(self) -> Result<PlannedResource<'name, 'elevate, 'action>> {
        Ok(PlannedResource {
            name: self.name,
            elevate: self.elevate,
            status: self.action.plan(self.elevate)?,
        })
    }
}

#[derive(Debug)]
struct PlannedResource<'name, 'elevate, 'action> {
    name: MaybeOwned<'name, str>,
    elevate: &'elevate Elevate,
    status: ChangeStatus<'action>,
}

impl<'elevate> PlannedResource<'_, 'elevate, '_> {
    #[expect(clippy::unwrap_used, reason = "stdout writing should never fail")]
    fn apply(self, mut stdout: impl io::Write, elevate: &Elevate) -> Result<()> {
        match self.status {
            ChangeStatus::NeedsChange(plan) => {
                let name = self.name.as_ref();

                write!(stdout, "Applying \"{name}\" ... ").unwrap();
                stdout.flush().unwrap();

                match plan.apply(elevate) {
                    Ok(()) => {
                        writeln!(stdout, "{}", "[DONE]".fg::<colors::Green>()).unwrap();
                        Ok(())
                    }
                    Err(err) => {
                        writeln!(stdout, "{}", "[FAILED]".fg::<colors::Red>()).unwrap();
                        Err(err)
                    }
                }
            }
            ChangeStatus::Clean => Ok(()),
        }
    }

    fn report(&'elevate self) -> Result<Report<'elevate>> {
        Ok(Report {
            name: &self.name,
            change: match self.status {
                ChangeStatus::NeedsChange(ref applyable) => {
                    let report = applyable.report()?;
                    StatusReport::Changes(report)
                }
                ChangeStatus::Clean => StatusReport::Unchanged,
            },
        })
    }
}

pub trait Plannable<'action>: fmt::Debug {
    fn plan<'elevate>(self: Box<Self>, elevate: &'elevate Elevate) -> Result<ChangeStatus<'action>>
    where
        'elevate: 'action;
}

pub trait Applyable: Reporter + fmt::Debug {
    fn apply(&self, elevate: &Elevate) -> Result<()>;
}

#[derive(Debug)]
struct ChecksumChange {
    from: Checksum,
    to: Checksum,
}

#[derive(PartialEq, Eq, Clone, derive_more::Display, Debug)]
pub struct Uid(String);

impl Uid {
    pub fn root() -> Self {
        Self("root".into())
    }

    pub fn from(s: String) -> Self {
        Self(s)
    }
}

#[derive(PartialEq, Eq, Clone, derive_more::Display, Debug)]
pub struct Gid(String);

impl Gid {
    pub fn root() -> Self {
        Self("root".into())
    }

    pub fn from(s: String) -> Self {
        Self(s)
    }
}

struct ActionRef<'a> {
    action: &'a (dyn Plannable<'a> + 'a),
}

pub struct ResourcePlanRef<'plan, 'name, 'elevate, 'action> {
    resource: &'plan mut Resource<'name, 'elevate, 'action>,
}

impl<'plan, 'name, 'elevate, 'action> ResourcePlanRef<'plan, 'name, 'elevate, 'action> {
    pub fn on_change(
        &'plan mut self,
        f: Box<dyn FnOnce() -> Result<Resource<'name, 'elevate, 'action>> + 'elevate>,
    ) -> Result<()> {
        self.resource.triggers.push(Trigger::OnChange(f));
        Ok(())
    }
}

#[derive(Debug)]
pub struct Plan<'name, 'elevate, 'pkgs, 'action> {
    resources: Vec<Resource<'name, 'elevate, 'action>>,
    managed_packages: Vec<MaybeOwned<'pkgs, str>>,
}

impl<'name, 'elevate, 'pkgs, 'action> Plan<'name, 'elevate, 'pkgs, 'action>
where
    'elevate: 'action,
{
    pub fn managed_packages(&self) -> &[MaybeOwned<'pkgs, str>] {
        &self.managed_packages
    }

    pub fn new() -> Self {
        Self {
            resources: vec![],
            managed_packages: vec![],
        }
    }

    pub fn add<'plan>(
        &'plan mut self,
        name: impl Into<MaybeOwned<'name, str>>,
        elevate: &'elevate Elevate,
        action: Box<dyn Plannable<'action> + 'action>,
    ) -> Result<ResourcePlanRef<'plan, 'name, 'elevate, 'action>> {
        let mut resource = Resource {
            name: name.into(),
            elevate,
            action,
            triggers: vec![],
        };

        let resource_ref = self.resources.push_mut(resource);

        let reference = ResourcePlanRef {
            resource: resource_ref,
        };

        Ok(reference)
    }

    pub fn register_packages<'a: 'pkgs>(
        &mut self,
        packages: &MaybeOwned<'a, [MaybeOwned<'a, str>]>,
    ) -> Result<()> {
        self.managed_packages.extend_from_slice(packages.as_ref());
        Ok(())
    }

    pub fn finish(self) -> Result<PlanFinished<'name, 'elevate, 'action>> {
        let mut actions = vec![];

        for mut resource in self.resources {
            // swap the triggers out
            let mut triggers = vec![];
            mem::swap(&mut resource.triggers, &mut triggers);

            let planned_resource = resource.plan()?;
            let changed = planned_resource.status.changed();

            for trigger in triggers {
                match trigger {
                    Trigger::OnChange(f) => {
                        if changed {
                            let triggered_resource = f()?;
                            let planned_triggered_resource = triggered_resource.plan()?;
                            actions.push(planned_triggered_resource);
                        }
                    }
                }
            }

            actions.push(planned_resource);
        }

        Ok(PlanFinished::<'name, 'elevate, 'action> { actions })
    }
}

pub struct PlanFinished<'name, 'elevate, 'action> {
    actions: Vec<PlannedResource<'name, 'elevate, 'action>>,
}

enum StatusReport {
    Changes(Vec<String>),
    Unchanged,
}

struct Report<'a> {
    name: &'a MaybeOwned<'a, str>,
    change: StatusReport,
}

#[must_use]
pub struct Reports<'a> {
    actions: Vec<Report<'a>>,
    all: usize,
    changed: usize,
}

#[expect(clippy::unwrap_used, reason = "stdout writing should never fail")]
impl<'a> Reports<'a> {
    pub fn print(&'a self, config: &ReportConfig) {
        let mut stdout = io::stdout();

        let mut action_printed = false;

        for action in &self.actions {
            match action.change {
                StatusReport::Changes(ref changes) => {
                    writeln!(
                        stdout,
                        "{} {name}",
                        "Resource [CHANGE]".fg::<colors::Yellow>(),
                        name = action.name.as_ref()
                    )
                    .unwrap();
                    for change in changes {
                        writeln!(stdout, "    > {change}").unwrap();
                    }
                    action_printed = true;
                }
                StatusReport::Unchanged => {
                    if *config == ReportConfig::All {
                        writeln!(
                            stdout,
                            "{}  {name}",
                            "Resource [CLEAN]".fg::<colors::Green>(),
                            name = action.name.as_ref()
                        )
                        .unwrap();
                        action_printed = true;
                    }
                }
            }
        }

        if action_printed {
            writeln!(stdout).unwrap();
        }

        match self.changed {
            0 => {
                write!(stdout, "{}, ", "Nothing to do".fg::<colors::Green>()).unwrap();
                match self.all {
                    0 => write!(stdout, "no resources defined").unwrap(),
                    all => {
                        write!(stdout, "{all} ").unwrap();
                        match all {
                            1 => write!(stdout, "resource").unwrap(),
                            _ => write!(stdout, "resources").unwrap(),
                        }
                        write!(stdout, " clean").unwrap();
                    }
                }
            }
            changes => {
                write!(stdout, "{}", format!("{changes} ").fg::<colors::Yellow>()).unwrap();
                match changes {
                    1 => write!(stdout, "{}", "resource ".fg::<colors::Yellow>()).unwrap(),
                    _ => write!(stdout, "{}", "resources ".fg::<colors::Yellow>()).unwrap(),
                }
                let unchanged = self.all.checked_sub(self.changed).expect("all >= changed");
                write!(
                    stdout,
                    "{} ({unchanged} ",
                    "to change".fg::<colors::Yellow>()
                )
                .unwrap();
                match unchanged {
                    1 => write!(stdout, "resource").unwrap(),
                    _ => write!(stdout, "resources").unwrap(),
                }

                write!(stdout, " clean)").unwrap();
            }
        }
        writeln!(stdout).unwrap();
    }

    pub fn has_changes(&self) -> bool {
        self.changed != 0
    }
}

#[derive(Eq, PartialEq)]
pub enum ReportConfig {
    All,
    OnlyChanged,
}

impl<'name, 'elevate, 'action> PlanFinished<'name, 'elevate, 'action> {
    pub fn report(&'elevate self) -> Result<Reports<'elevate>> {
        let mut action_reports = vec![];

        for action in &self.actions {
            let report = action.report()?;
            action_reports.push(report);
        }

        Ok(Reports {
            actions: action_reports,
            all: self.actions.len(),
            changed: self
                .actions
                .iter()
                .filter(|action| action.status.changed())
                .count(),
        })
    }

    pub fn approve(self) -> PlanApproved<'name, 'elevate, 'action> {
        PlanApproved::<'name, 'elevate, 'action> {
            actions: self.actions,
        }
    }
}

pub struct PlanApproved<'name, 'elevate, 'action> {
    actions: Vec<PlannedResource<'name, 'elevate, 'action>>,
}

impl PlanApproved<'_, '_, '_> {
    #[expect(clippy::unwrap_used, reason = "stdout writing should never fail")]
    pub fn apply(self) -> Result<()> {
        let mut stdout = io::stdout();
        writeln!(stdout).unwrap();
        for action in self.actions {
            let elevate = action.elevate;
            action.apply(&mut stdout, elevate)?;
        }

        Ok(())
    }
}
