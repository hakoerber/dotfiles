#![allow(unused, reason = "dev")]
#![deny(unused_must_use)]

use std::{
    fmt,
    io::{self, Write},
    mem,
    sync::{Arc, Mutex},
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
    pub use super::cmd::{Process, Stdin};
    pub use super::fs::{
        FileContent::{self, Lines},
        FileMode, FilePermissions, files_in_dir,
    };
    pub use super::gpg::{GpgFingerprint, GpgTrust};
    pub use super::maybeowned::MaybeOwned::{self, Owned, Ref};
    pub use super::pkg::{
        InstallReason, check_installed, get_package_version, list_explicitly_installed_packages,
        list_orphaned_packages, resolve_installed_package,
    };
    pub use super::resources::prelude::*;
    pub use super::tempfile::Tempfile;
    pub use super::{
        Elevate, ElevateTo, FinishResult, Gid, Path, PathBuf, Plan, ReportConfig, Resource,
        Uid,
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
pub enum ChangeStatus<'action> {
    NeedsChange(Box<dyn Applyable + 'action + Send + Sync>),
    Clean,
}

impl ChangeStatus<'_> {
    fn changed(&self) -> bool {
        matches!(self, Self::NeedsChange(_))
    }
}

#[derive(Debug)]
pub enum AssertionResult {
    Success,
    Failed(AssertionFailureReport),
}

enum Trigger<'name, 'elevate, 'action> {
    OnChange(Box<dyn FnOnce() -> Resource<'name, 'elevate, 'action> + Send + 'elevate>),
}

impl fmt::Debug for Trigger<'_, '_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::OnChange(ref _arg) => f.debug_tuple("OnChange").field(&"[fun]").finish(),
        }
    }
}

#[derive(Debug)]
pub struct Block<'name, 'elevate, 'action> {
    name: MaybeOwned<'name, str>,
    nodes: Vec<Node<'name, 'elevate, 'action>>,
    triggers: Vec<Trigger<'name, 'elevate, 'action>>,
}

impl<'name, 'elevate, 'action> Block<'name, 'elevate, 'action> {
    pub fn add<'a>(
        &'a mut self,
        name: impl Into<MaybeOwned<'name, str>>,
        elevate: &'elevate Elevate,
        action: impl Plannable<'action> + Send + 'action,
    ) -> NodePlanRef<'a, 'name, 'elevate, 'action> {
        let resource = Resource {
            name: name.into(),
            elevate,
            action: Box::new(action),
            triggers: vec![],
        };

        let resource_ref = self.nodes.push_mut(Node::Leaf(resource));

        NodePlanRef { node: resource_ref }
    }

    pub fn block<'plan, F>(
        &'plan mut self,
        name: impl Into<MaybeOwned<'name, str>>,
        f: F,
    ) -> NodePlanRef<'plan, 'name, 'elevate, 'action>
    where
        F: FnOnce(&mut Self),
    {
        let mut block = Block {
            name: name.into(),
            nodes: vec![],
            triggers: vec![],
        };

        f(&mut block);

        let resource_ref = self.nodes.push_mut(Node::Block(block));

        NodePlanRef { node: resource_ref }
    }

    pub fn with<'block, F, R>(&'block mut self, elevate: &'elevate Elevate, f: F) -> R
    where
        F: FnOnce(BlockAddWithElevate<'block, 'name, 'elevate, 'action>) -> R,
    {
        let plan = BlockAddWithElevate {
            elevate,
            block: self,
        };

        f(plan)
    }
}

#[derive(Debug)]
pub struct BlockAddWithElevate<'block, 'name, 'elevate, 'action> {
    elevate: &'elevate Elevate,
    block: &'block mut Block<'name, 'elevate, 'action>,
}

impl<'name, 'elevate, 'action> BlockAddWithElevate<'_, 'name, 'elevate, 'action> {
    pub fn add<'a>(
        &'a mut self,
        name: impl Into<MaybeOwned<'name, str>>,
        action: impl Plannable<'action> + Send + 'action,
    ) -> NodePlanRef<'a, 'name, 'elevate, 'action> {
        let resource = Resource {
            name: name.into(),
            elevate: self.elevate,
            action: Box::new(action),
            triggers: vec![],
        };

        let resource_ref = self.block.nodes.push_mut(Node::Leaf(resource));

        NodePlanRef { node: resource_ref }
    }
}

#[derive(Debug)]
pub struct Assertion<'name, 'elevate, 'assert> {
    name: MaybeOwned<'name, str>,
    elevate: &'elevate Elevate,
    assert: Box<dyn Assertable<'assert> + Send + 'assert>,
}

#[derive(Debug)]
enum Node<'name, 'elevate, 'action> {
    Block(Block<'name, 'elevate, 'action>),
    Leaf(Resource<'name, 'elevate, 'action>),
}

#[derive(Debug)]
pub struct Resource<'name, 'elevate, 'action> {
    name: MaybeOwned<'name, str>,
    elevate: &'elevate Elevate,
    action: Box<dyn Plannable<'action> + Send + 'action>,
    triggers: Vec<Trigger<'name, 'elevate, 'action>>,
}

impl<'name, 'elevate: 'action, 'action> Resource<'name, 'elevate, 'action> {
    pub fn new(
        name: MaybeOwned<'name, str>,
        elevate: &'elevate Elevate,
        action: impl Plannable<'action> + Send + 'action,
    ) -> Self {
        Self {
            name,
            elevate,
            action: Box::new(action),
            triggers: vec![],
        }
    }

    async fn plan(self) -> Result<PlannedResource<'name, 'elevate, 'action>> {
        Ok(PlannedResource {
            name: self.name,
            elevate: self.elevate,
            status: self.action.plan(self.elevate).await?,
        })
    }
}

#[derive(Debug)]
pub struct PlannedBlock<'name, 'elevate, 'action> {
    name: MaybeOwned<'name, str>,
    nodes: Vec<PlannedNode<'name, 'elevate, 'action>>,
}

impl PlannedBlock<'_, '_, '_> {
    fn has_changes(&self) -> bool {
        let mut has_changes = false;
        for node in &self.nodes {
            has_changes = has_changes || node.has_changes();
        }
        has_changes
    }
}

#[derive(Debug)]
enum PlannedNodeInner<'name, 'elevate, 'action> {
    Block(PlannedBlock<'name, 'elevate, 'action>),
    Leaf(PlannedResource<'name, 'elevate, 'action>),
}

#[derive(Debug)]
struct PlannedNode<'name, 'elevate, 'action> {
    inner: PlannedNodeInner<'name, 'elevate, 'action>,
}

impl PlannedNode<'_, '_, '_> {
    fn has_changes(&self) -> bool {
        match self.inner {
            PlannedNodeInner::Block(ref block) => block.has_changes(),
            PlannedNodeInner::Leaf(ref resource) => resource.status.changed(),
        }
    }
}

#[derive(Debug)]
struct PlannedResource<'name, 'elevate, 'action> {
    name: MaybeOwned<'name, str>,
    elevate: &'elevate Elevate,
    status: ChangeStatus<'action>,
}

impl<'name, 'elevate: 'name> PlannedResource<'name, 'elevate, '_> {
    #[expect(clippy::unwrap_used, reason = "stdout writing should never fail")]
    async fn apply(&self, mut stdout: impl io::Write, elevate: &Elevate) -> Result<()> {
        match self.status {
            ChangeStatus::NeedsChange(ref plan) => {
                let name = self.name.as_ref();

                write!(stdout, "Applying \"{name}\" ... ").unwrap();
                stdout.flush().unwrap();

                match plan.apply(elevate).await {
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

    fn report(&self) -> Result<Report<'_>> {
        Ok(Report {
            name: &self.name,
            status: match self.status {
                ChangeStatus::NeedsChange(ref applyable) => {
                    let report = applyable.report()?;
                    StatusReport::Changes(report)
                }
                ChangeStatus::Clean => StatusReport::Unchanged,
            },
        })
    }
}

#[async_trait::async_trait]
pub trait Plannable<'action>: fmt::Debug {
    async fn plan<'elevate>(
        self: Box<Self>,
        elevate: &'elevate Elevate,
    ) -> Result<ChangeStatus<'action>>
    where
        'elevate: 'action;
}

#[async_trait::async_trait]
pub trait Applyable: Reporter + fmt::Debug {
    async fn apply(&self, elevate: &Elevate) -> Result<()>;
}

#[async_trait::async_trait]
pub trait Assertable<'assert>: fmt::Debug {
    async fn assert<'elevate>(
        self: Box<Self>,
        elevate: &'elevate Elevate,
    ) -> Result<AssertionResult>
    where
        'elevate: 'assert;
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

pub struct NodePlanRef<'plan, 'name, 'elevate, 'action> {
    node: &'plan mut Node<'name, 'elevate, 'action>,
}

impl<'plan, 'name, 'elevate, 'action> NodePlanRef<'plan, 'name, 'elevate, 'action> {
    pub fn on_change<F>(&'plan mut self, f: F)
    where
        F: FnOnce() -> Resource<'name, 'elevate, 'action> + Send + 'elevate,
    {
        let new_trigger = Trigger::OnChange(Box::new(f));
        match *self.node {
            Node::Block(ref mut block) => block.triggers.push(new_trigger),
            Node::Leaf(ref mut resource) => resource.triggers.push(new_trigger),
        }
    }
}

#[derive(Debug)]
pub struct PlanAddWithElevate<'plan, 'name, 'elevate, 'pkgs, 'action> {
    elevate: &'elevate Elevate,
    plan: &'plan mut Plan<'name, 'elevate, 'pkgs, 'action>,
}

impl<'name, 'elevate, 'pkgs, 'action> PlanAddWithElevate<'_, 'name, 'elevate, 'pkgs, 'action> {
    pub fn add<'a>(
        &'a mut self,
        name: impl Into<MaybeOwned<'name, str>>,
        action: impl Plannable<'action> + Send + 'action,
    ) -> NodePlanRef<'a, 'name, 'elevate, 'action> {
        let resource = Resource {
            name: name.into(),
            elevate: self.elevate,
            action: Box::new(action),
            triggers: vec![],
        };

        let resource_ref = self.plan.nodes.push_mut(Node::Leaf(resource));

        NodePlanRef { node: resource_ref }
    }

    pub fn register_packages<'a: 'pkgs>(
        &mut self,
        packages: &MaybeOwned<'a, [MaybeOwned<'a, str>]>,
    ) {
        self.plan
            .managed_packages
            .extend_from_slice(packages.as_ref());
    }
}

#[derive(Debug)]
pub struct AssertionFailure<'name> {
    name: MaybeOwned<'name, str>,
    report: AssertionFailureReport,
}

pub enum FinishResult<'name, 'elevate, 'pkgs, 'action> {
    AssertionsFailed { report: AssertionReports<'name> },
    Success(PlanFinished<'name, 'elevate, 'pkgs, 'action>),
}

#[derive(Debug)]
pub struct Plan<'name, 'elevate, 'pkgs, 'action, 'secrets, S>
where
    S: fmt::Debug,
{
    nodes: Vec<Node<'name, 'elevate, 'action>>,
    assertions: Vec<Assertion<'name, 'elevate, 'action>>,
    managed_packages: Vec<MaybeOwned<'pkgs, str>>,
}

impl<'name, 'elevate, 'pkgs, 'action> Plan<'name, 'elevate, 'pkgs, 'action>
where
    'elevate: 'action,
{
    pub fn new() -> Self {
        Self {
            nodes: vec![],
            managed_packages: vec![],
            assertions: vec![],
        }
    }

    pub fn with<'plan, F, R>(&'plan mut self, elevate: &'elevate Elevate, f: F) -> R
    where
        F: FnOnce(PlanAddWithElevate<'plan, 'name, 'elevate, 'pkgs, 'action>) -> R,
    {
        let plan = PlanAddWithElevate::<'plan, '_, '_, '_, '_> {
            elevate,
            plan: self,
        };

        f(plan)
    }

    pub fn assert<'plan>(
        &'plan mut self,
        name: impl Into<MaybeOwned<'name, str>>,
        elevate: &'elevate Elevate,
        assertion: impl Assertable<'action> + Send + 'action,
    ) {
        let assertion = Assertion {
            name: name.into(),
            elevate,
            assert: Box::new(assertion),
        };
        self.assertions.push(assertion);
    }

    pub fn add<'plan>(
        &'plan mut self,
        name: impl Into<MaybeOwned<'name, str>>,
        elevate: &'elevate Elevate,
        action: impl Plannable<'action> + Send + 'action,
    ) -> NodePlanRef<'plan, 'name, 'elevate, 'action> {
        let resource = Resource {
            name: name.into(),
            elevate,
            action: Box::new(action),
            triggers: vec![],
        };

        let node = self.nodes.push_mut(Node::Leaf(resource));

        NodePlanRef { node }
    }

    pub fn block<'plan, F>(
        &'plan mut self,
        name: impl Into<MaybeOwned<'name, str>>,
        f: F,
    ) -> NodePlanRef<'plan, 'name, 'elevate, 'action>
    where
        F: FnOnce(&mut Block<'name, 'elevate, 'action>),
    {
        let mut block = Block {
            name: name.into(),
            nodes: vec![],
            triggers: vec![],
        };

        f(&mut block);

        let node_ref = self.nodes.push_mut(Node::Block(block));

        NodePlanRef { node: node_ref }
    }

    pub fn register_packages<'a: 'pkgs>(&mut self, packages: &[MaybeOwned<'a, str>]) {
        self.managed_packages.extend_from_slice(packages);
    }

    pub async fn finish(self) -> Result<FinishResult<'name, 'elevate, 'pkgs, 'action>> {
        let mut assertion_failures = vec![];
        for assertion in self.assertions {
            let assert_result = assertion.assert.assert(assertion.elevate).await?;
            match assert_result {
                AssertionResult::Success => (),
                AssertionResult::Failed(report) => {
                    assertion_failures.push(AssertionFailure {
                        name: assertion.name,
                        report,
                    });
                }
            }
        }

        if !assertion_failures.is_empty() {
            Ok(FinishResult::AssertionsFailed {
                report: AssertionReports { assertion_failures },
            })
        } else if cfg!(true) {
            let mut actions = vec![];

            for node in self.nodes {
                actions.extend(resolve_node(node).await?);
            }

            Ok(FinishResult::Success(PlanFinished {
                nodes: actions,
                managed_packages: self.managed_packages,
            }))
        } else {
            let results = Arc::new(Mutex::new(vec![]));
            let errors = Arc::new(Mutex::new(vec![]));
            let ((), _tasks) = async_scoped::TokioScope::scope_and_block(|scope| {
                for node in self.nodes {
                    let results = Arc::clone(&results);
                    let errors = Arc::clone(&errors);

                    scope.spawn(async move {
                        match resolve_node(node).await {
                            Ok(action) => results.lock().expect("lock unpoisoned").push(action),
                            Err(err) => errors.lock().expect("lock unpoisoned").push(err),
                        }
                    });
                }
            });

            let mut errors = Arc::into_inner(errors)
                .expect("only a single reference remaning")
                .into_inner()
                .expect("mutex unpoisoned");

            if let Some(err) = errors.pop() {
                Err(err)
            } else {
                // unwrap Arc/Mutex
                let actions = Arc::into_inner(results)
                    .expect("only a single reference remaning")
                    .into_inner()
                    .expect("mutex unpoisoned");

                // flatten the outer (all tasks) and inner (per task) vecs
                let actions = actions.into_iter().flatten().collect();

                Ok(FinishResult::Success(PlanFinished {
                    nodes: actions,
                    managed_packages: self.managed_packages,
                }))
            }
        }
    }
}

async fn resolve_node<'name, 'elevate, 'action>(
    node: Node<'name, 'elevate, 'action>,
) -> Result<Vec<PlannedNode<'name, 'elevate, 'action>>>
where
    'elevate: 'action,
{
    let mut actions = vec![];

    match node {
        Node::Block(Block {
            name,
            nodes,
            triggers,
        }) => {
            let mut block_actions = vec![];
            let mut any_changed = false;

            for node in nodes {
                let result = Box::pin(resolve_node(node)).await?;
                any_changed = any_changed || result.iter().any(|node| node.has_changes());
                block_actions.extend(result);
            }

            for trigger in triggers {
                match trigger {
                    Trigger::OnChange(f) => {
                        if any_changed {
                            let triggered_resource = f();
                            let planned_triggered_resource = triggered_resource.plan().await?;

                            actions.push(PlannedNode {
                                inner: PlannedNodeInner::Leaf(planned_triggered_resource),
                            });
                        }
                    }
                }
            }

            let out = PlannedNode {
                inner: PlannedNodeInner::Block(PlannedBlock {
                    name,
                    nodes: block_actions,
                }),
            };

            actions.push(out);
        }
        Node::Leaf(mut resource) => {
            // swap the triggers out
            let mut triggers = vec![];
            mem::swap(&mut resource.triggers, &mut triggers);

            let planned_resource = resource.plan().await?;
            let changed = planned_resource.status.changed();
            actions.push(PlannedNode {
                inner: PlannedNodeInner::Leaf(planned_resource),
            });

            for trigger in triggers {
                match trigger {
                    Trigger::OnChange(f) => {
                        if changed {
                            let triggered_resource = f();
                            let planned_triggered_resource = triggered_resource.plan().await?;
                            actions.push(PlannedNode {
                                inner: PlannedNodeInner::Leaf(planned_triggered_resource),
                            });
                        }
                    }
                }
            }
        }
    }
    Ok(actions)
}

pub struct PlanFinished<'name, 'elevate, 'pkgs, 'action> {
    nodes: Vec<PlannedNode<'name, 'elevate, 'action>>,
    managed_packages: Vec<MaybeOwned<'pkgs, str>>,
}

#[derive(Debug)]
enum StatusReport {
    Changes(Vec<String>),
    Unchanged,
}

impl StatusReport {
    fn is_changed(&self) -> bool {
        matches!(self, Self::Changes(_))
    }
}

#[derive(Debug)]
struct Report<'name> {
    name: &'name MaybeOwned<'name, str>,
    status: StatusReport,
}

#[derive(Debug)]
pub struct AssertionFailureReport {
    reason: Vec<String>,
}

#[must_use]
pub struct AssertionReports<'name> {
    assertion_failures: Vec<AssertionFailure<'name>>,
}

#[expect(clippy::unwrap_used, reason = "stdout writing should never fail")]
impl AssertionReports<'_> {
    pub fn print(&self) {
        let mut stdout = io::stdout();

        writeln!(stdout, "{}", "Assertions failed".fg::<colors::Red>()).unwrap();

        for failure in &self.assertion_failures {
            match failure.report.reason[..] {
                [ref single_line] => writeln!(
                    stdout,
                    "{}: {}",
                    failure.name.as_ref().fg::<colors::Red>(),
                    single_line
                )
                .unwrap(),
                ref lines => {
                    writeln!(stdout, "{}", failure.name.as_ref().fg::<colors::Red>()).unwrap();
                    for line in lines {
                        writeln!(stdout, "    {line}").unwrap();
                    }
                }
            }
        }
    }
}

#[must_use]
pub struct Reports<'a> {
    nodes: Vec<NodeReport<'a>>,
    all: usize,
    changes: usize,
}

#[expect(clippy::unwrap_used, reason = "stdout writing should never fail")]
fn print_node(
    node: &NodeReport,
    stdout: &mut impl io::Write,
    config: &ReportConfig,
    first: bool,
    last: bool,
    prefix: &str,
) -> bool {
    match *node {
        NodeReport::Resource { ref report } => {
            let mut action_printed = false;

            match report.status {
                StatusReport::Changes(ref changes) => {
                    writeln!(
                        stdout,
                        "{prefix}{c}─ {} {name}",
                        "Resource [CHANGE]".fg::<colors::Yellow>(),
                        prefix = prefix.fg::<colors::White>(),
                        c = if first && last {
                            // only a single resource, no lines to draw
                            '•'
                        } else if first {
                            '╭'
                        } else if last {
                            '╰'
                        } else {
                            '├'
                        },
                        name = report.name.as_ref()
                    )
                    .unwrap();

                    let detail_prefix = if last {
                        format!("{prefix}   ")
                    } else {
                        format!("{prefix}│  ")
                    };

                    for change in changes {
                        writeln!(
                            stdout,
                            "{detail_prefix} > {change}",
                            detail_prefix = detail_prefix.fg::<colors::White>()
                        )
                        .unwrap();
                    }

                    if !last {
                        writeln!(stdout, "{prefix}│", prefix = prefix.fg::<colors::White>())
                            .unwrap();
                    }

                    action_printed = true;
                }
                StatusReport::Unchanged => {
                    if *config == ReportConfig::All {
                        writeln!(
                            stdout,
                            "{}  {name}",
                            "Resource [CLEAN]".fg::<colors::Green>(),
                            name = report.name.as_ref()
                        )
                        .unwrap();
                        action_printed = true;
                    }
                }
            }

            action_printed
        }
        NodeReport::Block {
            ref name,
            ref nodes,
            count: _,
            changes: _,
        } => {
            writeln!(
                stdout,
                "{prefix}{c}──┬─ {} {name}",
                "Block".fg::<colors::Yellow>(),
                c = if last { '╰' } else { '├' },
                name = name.as_ref()
            )
            .unwrap();

            let nodes: Vec<_> = if *config == ReportConfig::All {
                nodes.iter().collect()
            } else {
                nodes
                    .iter()
                    .filter(|node| match **node {
                        NodeReport::Resource { ref report } => report.status.is_changed(),
                        NodeReport::Block {
                            name: _,
                            nodes: _,
                            count: _,
                            changes,
                        } => changes != 0,
                    })
                    .collect()
            };

            let prefix = format!("{prefix}{add}", add = if last { "   " } else { "│  " });

            writeln!(stdout, "{prefix}│", prefix = prefix.fg::<colors::White>()).unwrap();

            for (i, node) in nodes.iter().enumerate() {
                print_node(
                    node,
                    stdout,
                    config,
                    false,
                    i == nodes
                        .len()
                        .checked_sub(1)
                        .expect("if len < 1, this loop would never be taken"),
                    &prefix,
                );
            }

            if !last {
                writeln!(stdout, "{prefix}", prefix = prefix.fg::<colors::White>()).unwrap();
            }
            true // blocks always print something
        }
    }
}

#[expect(clippy::unwrap_used, reason = "stdout writing should never fail")]
impl<'a> Reports<'a> {
    pub fn print(&'a self, config: &ReportConfig) {
        let mut stdout = io::stdout();

        let mut action_printed = false;

        let nodes: Vec<_> = if *config == ReportConfig::All {
            self.nodes.iter().collect()
        } else {
            self.nodes
                .iter()
                .filter(|node| match **node {
                    NodeReport::Resource { ref report } => report.status.is_changed(),
                    NodeReport::Block {
                        name: _,
                        nodes: _,
                        count: _,
                        changes,
                    } => changes != 0,
                })
                .collect()
        };

        let len = nodes.len();

        for (i, node) in nodes.into_iter().enumerate() {
            action_printed = print_node(
                node,
                &mut stdout,
                config,
                i == 0,
                i == len
                    .checked_sub(1)
                    .expect("if len < 1, this loop would never be taken"),
                "",
            ) || action_printed;
        }

        if action_printed {
            writeln!(stdout).unwrap();
        }

        match self.changes {
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
                let unchanged = self.all.checked_sub(self.changes).expect("all >= changed");
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
        self.changes != 0
    }
}

#[derive(Eq, PartialEq)]
pub enum ReportConfig {
    All,
    OnlyChanged,
}

#[derive(Debug)]
enum NodeReport<'name> {
    Resource {
        report: Report<'name>,
    },
    Block {
        name: &'name MaybeOwned<'name, str>,
        nodes: Vec<Self>,
        count: usize,
        changes: usize,
    },
}

fn report_node<'node, 'name, 'elevate, 'action>(
    node: &'node PlannedNode<'name, 'elevate, 'action>,
) -> Result<Vec<NodeReport<'name>>>
where
    'node: 'name,
{
    let mut reports = vec![];

    match node.inner {
        PlannedNodeInner::Block(ref block) => {
            let mut nodes = vec![];
            for node in &block.nodes {
                let node_reports = report_node(node)?;
                nodes.extend(node_reports);
            }
            let node_count = nodes
                .iter()
                .map(|report| match *report {
                    NodeReport::Resource { report: _ } => 1,
                    NodeReport::Block {
                        name: _,
                        nodes: _,
                        count,
                        changes: _,
                    } => count,
                })
                .sum();
            let changes = nodes
                .iter()
                .map(|report| match *report {
                    NodeReport::Resource { ref report } => {
                        if report.status.is_changed() {
                            1
                        } else {
                            0
                        }
                    }
                    NodeReport::Block {
                        name: _,
                        nodes: _,
                        count,
                        changes: _,
                    } => count,
                })
                .sum();
            reports.push(NodeReport::Block {
                name: &block.name,
                nodes,
                count: node_count,
                changes,
            });
        }
        PlannedNodeInner::Leaf(ref resource) => {
            let report = resource.report()?;
            reports.push(NodeReport::Resource { report });
        }
    }

    Ok(reports)
}

impl<'name, 'elevate, 'pkgs, 'action> PlanFinished<'name, 'elevate, 'pkgs, 'action> {
    pub fn report(&'elevate self) -> Result<Reports<'elevate>> {
        let mut reports = vec![];

        for node in &self.nodes {
            reports.extend(report_node(node)?);
        }

        let count = reports
            .iter()
            .map(|report| match *report {
                NodeReport::Resource { report: _ } => 1,
                NodeReport::Block {
                    name: _,
                    nodes: _,
                    count,
                    changes: _,
                } => count,
            })
            .sum();

        let changes = reports
            .iter()
            .map(|report| match *report {
                NodeReport::Resource { ref report } => {
                    if report.status.is_changed() {
                        1
                    } else {
                        0
                    }
                }
                NodeReport::Block {
                    name: _,
                    nodes: _,
                    count: _,
                    changes,
                } => changes,
            })
            .sum();

        Ok(Reports {
            nodes: reports,
            all: count,
            changes,
        })
    }

    pub fn approve(self) -> PlanApproved<'name, 'elevate, 'pkgs, 'action> {
        PlanApproved::<'name, 'elevate, 'pkgs, 'action> {
            nodes: self.nodes,
            managed_packages: self.managed_packages,
        }
    }
}

pub struct PlanApproved<'name, 'elevate, 'pkgs, 'action> {
    nodes: Vec<PlannedNode<'name, 'elevate, 'action>>,
    managed_packages: Vec<MaybeOwned<'pkgs, str>>,
}

async fn apply_node(node: &PlannedNode<'_, '_, '_>, stdout: &mut impl io::Write) -> Result<()> {
    match node.inner {
        PlannedNodeInner::Block(ref block) => {
            for node in &block.nodes {
                Box::pin(apply_node(node, stdout)).await?;
            }
        }
        PlannedNodeInner::Leaf(ref resource) => {
            resource.apply(stdout, resource.elevate).await?;
        }
    }

    Ok(())
}

impl<'pkgs> PlanApproved<'_, '_, 'pkgs, '_> {
    pub fn managed_packages(&self) -> &[MaybeOwned<'pkgs, str>] {
        &self.managed_packages
    }
}

impl PlanApproved<'_, '_, '_, '_> {
    #[expect(clippy::unwrap_used, reason = "stdout writing should never fail")]
    pub async fn apply(&self) -> Result<()> {
        let mut stdout = io::stdout();
        writeln!(stdout).unwrap();
        for node in &self.nodes {
            apply_node(node, &mut stdout).await?;
        }

        Ok(())
    }
}
