#![allow(unused, reason = "dev")]
#![deny(unused_must_use)]

use std::{
    fmt,
    io::{self, Write},
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
    pub use super::resources::prelude::*;
    pub use super::{
        BaseContext, Context, Elevate, ElevateTo, Gid, InitialState, Path, PathBuf, Plan,
        ReportConfig, Uid,
    };
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
pub enum ChangeStatus<'ctx> {
    NeedsChange(Box<dyn Applyable + 'ctx>),
    Clean,
}

impl ChangeStatus<'_> {
    fn changed(&self) -> bool {
        matches!(self, Self::NeedsChange(_))
    }
}

#[derive(Debug)]
struct Action<'name, 'ctx> {
    name: MaybeOwned<'name, str>,
    status: ChangeStatus<'ctx>,
}

impl<'ctx> Action<'_, 'ctx> {
    #[expect(clippy::unwrap_used, reason = "stdout writing should never fail")]
    fn apply(self, mut stdout: impl io::Write) -> Result<()> {
        match self.status {
            ChangeStatus::NeedsChange(plan) => {
                let name = self.name.as_ref();

                write!(stdout, "Applying \"{name}\" ... ").unwrap();
                stdout.flush().unwrap();

                match plan.apply() {
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

    fn report(&'ctx self) -> Result<Report<'ctx>> {
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

pub trait Plannable<'change> {
    fn plan<'ctx>(self: Box<Self>, ctx: &'ctx Context<'ctx>) -> Result<ChangeStatus<'change>>
    where
        'ctx: 'change;
}

pub trait Applyable: Reporter + fmt::Debug {
    fn apply(&self) -> Result<()>;
}

#[derive(Debug)]
pub struct InitialState {
    pub installed_packages: Vec<String>,
}

impl InitialState {
    pub fn load(elevate: &Elevate) -> Result<Self> {
        Ok(Self {
            installed_packages: pkg::list_installed_packages(elevate)?,
        })
    }
}

#[derive(Debug)]
pub struct BaseContext<'c> {
    pub initial_state: &'c InitialState,
}

impl<'c> BaseContext<'c> {
    pub fn with_elevate(&'c self, elevate: Elevate) -> Context<'c> {
        Context {
            initial_state: self.initial_state,
            elevate,
        }
    }
}

#[derive(Debug)]
pub struct Context<'c> {
    pub elevate: Elevate,
    pub initial_state: &'c InitialState,
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

// struct ActionRef<'a> {
//     action: &'a (dyn Plannable<'a> + 'a),
// }

pub struct ActionPlanRef<'plan, 'name, 'ctx> {
    plan: &'plan mut Plan<'name, 'ctx>,
    changed: bool,
}

impl<'plan, 'name, 'ctx> ActionPlanRef<'plan, 'name, 'ctx> {
    pub fn on_change<F>(&'plan mut self, f: F) -> Result<()>
    where
        F: FnOnce(&'plan mut Plan<'name, 'ctx>) -> Result<()>,
    {
        if self.changed {
            f(self.plan)?;
        }
        Ok(())
    }

    pub fn changed(&self) -> bool {
        self.changed
    }
}

#[derive(Debug)]
pub struct Plan<'name, 'ctx> {
    actions: Vec<Action<'name, 'ctx>>,
}

impl<'name, 'ctx> Plan<'name, 'ctx> {
    pub fn new() -> Self {
        Self { actions: vec![] }
    }

    pub fn add<'a>(
        &'a mut self,
        name: impl Into<MaybeOwned<'name, str>>,
        ctx: &'ctx Context<'ctx>,
        action: Box<dyn Plannable<'ctx> + 'a>,
    ) -> Result<ActionPlanRef<'a, 'name, 'ctx>> {
        let status = action.plan(ctx)?;
        let changed = status.changed();

        let action = Action {
            name: name.into(),
            status,
        };

        self.actions.push(action);

        let r = ActionPlanRef {
            plan: self,
            changed,
        };

        Ok(r)
    }

    pub fn finish(self) -> PlanFinished<'name, 'ctx> {
        PlanFinished::<'name, 'ctx> {
            actions: self.actions,
        }
    }
}

pub struct PlanFinished<'name, 'ctx> {
    actions: Vec<Action<'name, 'ctx>>,
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

impl<'name, 'ctx> PlanFinished<'name, 'ctx> {
    pub fn report(&'ctx self) -> Result<Reports<'ctx>> {
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

    pub fn approve(self) -> PlanApproved<'name, 'ctx> {
        PlanApproved::<'name, 'ctx> {
            actions: self.actions,
        }
    }
}

pub struct PlanApproved<'name, 'ctx> {
    actions: Vec<Action<'name, 'ctx>>,
}

impl PlanApproved<'_, '_> {
    #[expect(clippy::unwrap_used, reason = "stdout writing should never fail")]
    pub fn apply(self) -> Result<()> {
        let mut stdout = io::stdout();
        writeln!(stdout).unwrap();
        for action in self.actions {
            action.apply(&mut stdout)?;
        }

        Ok(())
    }
}
