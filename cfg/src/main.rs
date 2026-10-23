#![allow(unused)]

use std::{
    env, fmt, fs,
    io::{self, Read, Write},
    os::unix::fs::{self as unixfs, MetadataExt as _, PermissionsExt},
    process::{self},
};

use anyhow::{Result, anyhow, bail};
use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use nonempty::nonempty;
use owo_colors::OwoColorize as _;
use regex::Regex;
use sha3::Digest as _;
use tap::Tap as _;

mod block_in_file;
mod checksum;
mod cmd;
mod file;
mod package;
mod tempfile;

use tempfile::Tempfile;

#[derive(Eq, PartialEq, Debug)]
struct Checksum(
    sha3::digest::array::Array<u8, <sha3::Sha3_256 as sha3::digest::OutputSizeUser>::OutputSize>,
);

impl fmt::Display for Checksum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0.as_slice() {
            write!(f, "{byte:x}")?;
        }

        Ok(())
    }
}

#[derive(Eq, PartialEq, Debug)]
enum ElevateTo {
    Root,
    User { user: String },
}

#[derive(Eq, PartialEq, Debug)]
enum Elevate {
    Sudo(ElevateTo),
    No,
}

#[derive(Debug)]
struct PacmanRefreshPackageDatabase;

// impl Report for PacmanRefreshPackageDatabase {
//     fn report(&self) {
//         println!("package database refreshed")
//     }
// }

// fn pacman_refresh_package_database(ctx: &Context) -> Result<Change<PacmanRefreshPackageDatabase>> {
//     cmd(&vec!["pacman", "--sync", "--refresh", "--"], &ctx.elevate)?;

//     Ok(Change::Changed(PacmanRefreshPackageDatabase))
// }

// fn install_packages<'pkg>(
//     ctx: &Context,
//     list: &[&'pkg str],
// ) -> Result<Change<PackageChange<'pkg>>> {
//     let mut installed: Vec<&str> = vec![];
//     for package in list {
//         if cmd(
//             &vec!["pacman", "--query", "--quiet", "--", package],
//             &ctx.elevate,
//         )?
//         .status
//         .code()
//             == 1
//         {
//             cmd(
//                 &vec!["pacman", "--needed", "--noconfirm", "--sync", "--", package],
//                 &ctx.elevate,
//             )?;
//             installed.push(package);
//         }
//     }

//     if let Some(installed) = NonEmpty::from_vec(installed) {
//         Ok(Change::Changed(PackageChange { installed }))
//     } else {
//         Ok(Change::Clean)
//     }
// }

// #[derive(Debug)]
// struct PackageChange<'pkg> {
//     installed: NonEmpty<&'pkg str>,
// }

// impl Report for PackageChange<'_> {
//     fn report(&self) {
//         for installed in &self.installed {
//             println!("installed {installed}");
//         }
//     }
// }

trait Reporter {
    fn report(&self) -> Vec<String>;
}

#[derive(Debug)]
enum ChangeStatus<'a> {
    NeedsChange(Box<dyn Applyable + 'a>),
    Clean,
}

#[derive(Debug)]
struct Change<'a> {
    name: &'a str,
    status: ChangeStatus<'a>,
}

impl<'a> ChangeStatus<'a> {
    fn is_changed(&self) -> bool {
        matches!(self, Self::NeedsChange(_))
    }
}

impl<'a> Change<'a> {
    fn apply(self) -> Result<()> {
        match self.status {
            ChangeStatus::NeedsChange(plan) => {
                plan.apply()?;
                Ok(())
            }
            ChangeStatus::Clean => Ok(()),
        }
    }

    fn report(&'a self) -> Report<'a> {
        Report {
            name: &self.name,
            change: match self.status {
                ChangeStatus::NeedsChange(ref applyable) => {
                    let report = applyable.report();
                    StatusReport::Changes(report)
                }
                ChangeStatus::Clean => StatusReport::Unchanged,
            },
        }
    }
}

trait Plannable<'a> {
    fn plan(self: Box<Self>, ctx: &'a Context<'a>) -> Result<ChangeStatus<'a>>;
}

trait Applyable: Reporter + fmt::Debug {
    fn apply(&self) -> Result<()>;
}

#[derive(Debug)]
struct Context<'c> {
    cachedir: &'c Path,
    elevate: Elevate,
}

#[derive(Debug)]
struct ChecksumChange {
    from: Checksum,
    to: Checksum,
}

#[derive(Debug)]
struct RawFileContent(Vec<u8>);

impl RawFileContent {
    fn new() -> Self {
        Self(vec![])
    }

    fn truncate(&mut self) {
        self.0.truncate(0);
    }

    fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    fn write_all(&mut self, buf: &[u8]) {
        self.0.write(buf).expect("memory writes to not fail");
    }
}

#[derive(Debug)]
struct FileContent<'a>(&'a [&'a str]);

impl FileContent<'_> {
    fn as_raw(&self) -> RawFileContent {
        let mut raw = RawFileContent::new();
        for line in self.0 {
            raw.write_all(line.as_bytes());
            raw.write_all(b"\n");
        }
        raw
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
struct FileMode(u32);
impl FileMode {
    fn as_oct_string(&self) -> String {
        format!("{0:04o}", self.0)
    }
}

#[derive(PartialEq, Eq, Clone, Copy, derive_more::Display, Debug)]
struct Uid(u32);

impl Uid {
    fn root() -> Self {
        Self(0)
    }

    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

#[derive(PartialEq, Eq, Clone, Copy, derive_more::Display, Debug)]
struct Gid(u32);

impl Gid {
    fn root() -> Self {
        Self(0)
    }

    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

#[derive(PartialEq, Eq, Debug)]
struct FilePermissions {
    owner: Uid,
    group: Gid,
    mode: FileMode,
}

#[derive(Debug)]
struct FilePermissionDiff {
    new_mode: Option<FileMode>,
    new_owner: Option<Uid>,
    new_group: Option<Gid>,
}

impl FilePermissionDiff {
    fn is_empty(&self) -> bool {
        self.new_mode.is_none() && self.new_owner.is_none() && self.new_group.is_none()
    }

    fn has_changes(&self) -> bool {
        !self.is_empty()
    }
}

impl FilePermissions {
    fn diff(&self, other: &Self) -> FilePermissionDiff {
        FilePermissionDiff {
            new_mode: (self.mode != other.mode).then_some(other.mode),
            new_owner: (self.owner != other.owner).then_some(other.owner),
            new_group: (self.group != other.group).then_some(other.group),
        }
    }
}

#[derive(Debug)]
struct FilePermissionChange {
    existing: FilePermissions,
    diff: FilePermissionDiff,
}

impl Reporter for FilePermissionChange {
    fn report(&self) -> Vec<String> {
        let mut out = vec![];
        if let Some(mode) = self.diff.new_mode {
            out.push(format!(
                "mode: {} -> {}",
                self.existing.mode.as_oct_string(),
                mode.as_oct_string()
            ));
        }
        if let Some(owner) = self.diff.new_owner {
            out.push(format!("owner: {} -> {}", self.existing.owner, owner));
        }
        if let Some(group) = self.diff.new_group {
            out.push(format!("group: {} -> {}", self.existing.group, group));
        }
        out
    }
}

#[derive(Debug)]
struct Plan<'a> {
    changes: Vec<Change<'a>>,
}

impl<'a> Plan<'a> {
    fn new() -> Self {
        Self { changes: vec![] }
    }

    fn add(
        &mut self,
        name: &'a str,
        ctx: &'a Context<'a>,
        action: Box<dyn Plannable<'a> + 'a>,
    ) -> Result<()> {
        let status = action.plan(ctx)?;
        let change = Change { name, status };
        self.changes.push(change);
        Ok(())
    }

    fn finish(self) -> PlanFinished<'a> {
        PlanFinished {
            changes: self.changes,
        }
    }
}

struct PlanFinished<'a> {
    changes: Vec<Change<'a>>,
}

enum StatusReport {
    Changes(Vec<String>),
    Unchanged,
}

struct Report<'a> {
    name: &'a str,
    change: StatusReport,
}

struct Reports<'a>(Vec<Report<'a>>);

impl<'a> Reports<'a> {
    fn print(&'a self) {
        for report in &self.0 {
            use owo_colors::OwoColorize;
            use owo_colors::colors::*;

            match report.change {
                StatusReport::Changes(ref changes) => {
                    println!(
                        "{} {name}",
                        "Resource [CHANGE]".fg::<Yellow>(),
                        name = report.name
                    );
                    for change in changes {
                        println!("    > {change}");
                    }
                }
                StatusReport::Unchanged => {
                    println!(
                        "{}  {name}",
                        "Resource [CLEAN]".fg::<Green>(),
                        name = report.name
                    );
                }
            }
        }
    }
}

impl<'a> PlanFinished<'a> {
    fn report(&'a self) -> Reports<'a> {
        let reports = {
            let mut reports = vec![];

            for change in &self.changes {
                let report = change.report();
                reports.push(report);
            }
            Reports(reports)
        };

        reports
    }

    fn approve(self) -> PlanApproved<'a> {
        PlanApproved {
            changes: self.changes,
        }
    }
}

struct PlanApproved<'a> {
    changes: Vec<Change<'a>>,
}

impl PlanApproved<'_> {
    fn apply(self) -> Result<()> {
        for change in self.changes {
            change.apply()?;
        }

        Ok(())
    }
}

fn main() -> Result<()> {
    let cachedir = PathBuf::from(env::var("XDG_CACHE_HOME")?);

    let sudo = Context {
        cachedir: &cachedir,
        elevate: Elevate::Sudo(ElevateTo::Root),
    };

    let with_current_user = Context {
        cachedir: &cachedir,
        elevate: Elevate::No,
    };

    let mut plan = Plan::new();

    plan.add(
        "pacman multilib",
        &sudo,
        Box::new(block_in_file::BlockInFile {
            path: Path::new("/etc/pacman.conf"),
            block: &["[multilib]", "Include = /etc/pacman.d/mirrorlist"],
            marker: "# MANAGED multilib",
            insert_after: None,
        }),
    )?;

    plan.add(
        "pacman parallel downloads",
        &sudo,
        Box::new(block_in_file::BlockInFile {
            path: Path::new("/etc/pacman.conf"),
            block: &["ParallelDownloads = 5"],
            marker: "# MANAGED parallel downloads",
            insert_after: Some(Regex::new("\\[options\\]").unwrap()),
        }),
    )?;

    plan.add(
        "pacman cache cleanup service",
        &sudo,
        Box::new(file::File {
            path: Path::new("/etc/systemd/system/pacman-cache-cleanup.service"),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o644),
            },
            content: FileContent(&[
                "[Service]",
                "Type=oneshot",
                "ExecStart=/bin/sh -c '/usr/bin/paccache -rk1 && /usr/bin/paccache -ruk0'",
                "RemainAfterExit=true",
            ]),
        }),
    )?;

    plan.add(
        "pacman cache cleanup timer",
        &sudo,
        Box::new(file::File {
            path: Path::new("/etc/systemd/system/pacman-cache-cleanup.timer"),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o644),
            },
            content: FileContent(&[
                "[Timer]",
                "OnCalendar=daily",
                "",
                "[Install]",
                "WantedBy=multi-user.target",
            ]),
        }),
    )?;

    plan.add(
        "install pacman-contrib",
        &sudo,
        Box::new(package::Packages {
            install: &["pacman-contrib", "xdot"],
        }),
    )?;

    let plan = plan.finish();

    let report = plan.report();
    report.print();

    let plan = plan.approve();

    plan.apply()?;

    // let multilib_change = block_in_file(&with_sudo)?.report("pacman multilib");

    // if multilib_change.is_changed() {
    //     pacman_refresh_package_database(&with_sudo)?.report("pacman database refresh");
    // }

    Ok(())
}
