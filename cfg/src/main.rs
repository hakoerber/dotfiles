#![allow(unused)]

use std::{
    env, fmt, fs,
    io::{self, Read, Write},
    os::unix::fs::{self as unixfs, MetadataExt as _, PermissionsExt},
    process::{self},
};

use anyhow::{Result, anyhow, bail};
use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use nonempty::{NonEmpty, nonempty};
use regex::Regex;
use sha3::Digest as _;
use tap::Tap as _;

mod block_in_file;
mod checksum;
mod tempfile;

use block_in_file::BlockInFile;

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

#[derive(Eq, PartialEq)]
enum ElevateTo {
    Root,
    User { user: String },
}

#[derive(Eq, PartialEq)]
enum Elevate {
    Sudo(ElevateTo),
    No,
}

#[derive(Debug)]
struct ExitStatus(usize);

impl ExitStatus {
    fn code(&self) -> usize {
        self.0
    }
}

#[derive(Debug)]
struct Process {
    stdout: String,
    stderr: String,
    status: ExitStatus,
}

fn cmd(args: &[&str], elevate: &Elevate) -> Result<Process> {
    let command = match *elevate {
        Elevate::Sudo(ref elevate_to) => match *elevate_to {
            ElevateTo::Root => vec!["sudo", "--"].tap_mut(|v| v.extend_from_slice(args)),
            ElevateTo::User { ref user } => {
                vec!["sudo", "-u", user, "--"].tap_mut(|v| v.extend_from_slice(args))
            }
        },
        Elevate::No => args.to_vec(),
    };

    let output = process::Command::new(command[0])
        .args(&command[1..])
        .output()?;

    Ok(Process {
        stdout: output.stdout.try_into()?,
        stderr: output.stderr.try_into()?,
        status: ExitStatus(
            output
                .status
                .code()
                .expect("process to have exit code on stop")
                .try_into()
                .expect("exit code to be non-negative"),
        ),
    })
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

trait Report {
    fn report(&self);
}

enum Change<'a> {
    Changed(Box<dyn Applyable + 'a>),
    Clean,
}

// impl<T> Change<T>
// where
//     T: Report + fmt::Debug,
// {
//     fn is_changed(&self) -> bool {
//         matches!(self, Self::Changed(_))
//     }

//     fn report(self, name: &str) -> Self {
//         match self {
//             Change::Changed(ref changes) => {
//                 println!("{name} changes");
//                 changes.report();
//             }
//             Change::Clean => println!("{name} unchanged"),
//         }
//         self
//     }
// }

trait Plannable<'a> {
    fn plan(self: Box<Self>, ctx: Box<Context<'a>>) -> Result<Change<'a>>;
}

trait Applyable {
    fn apply(&self) -> Result<()>;
}

fn move_file(from: &Path, to: &Path, elevate: &Elevate) -> Result<()> {
    match elevate {
        Elevate::Sudo(elevate_to) => {
            cmd(
                &[
                    "mv",
                    "--force",
                    "--no-target-directory",
                    "--",
                    from.as_str(),
                    to.as_str(),
                ],
                elevate,
            )?;
        }
        Elevate::No => fs::rename(from, to)?,
    }
    Ok(())
}

struct Context<'c> {
    cachedir: &'c Path,
    elevate: Elevate,
}

#[derive(Debug)]
struct ChecksumChange {
    from: Checksum,
    to: Checksum,
}

impl Report for ChecksumChange {
    fn report(&self) {
        println!("{} -> {}", self.from, self.to);
    }
}

#[derive(Debug)]
struct FileChangeDetails {
    checksum_change: Option<ChecksumChange>,
    permission_change: Option<FilePermissionChange>,
}

impl Report for FileChangeDetails {
    fn report(&self) {
        if let Some(ref checksum_change) = self.checksum_change {
            checksum_change.report()
        };
        if let Some(ref permission_change) = self.permission_change {
            permission_change.report()
        };
    }
}

#[derive(Debug)]
enum FileChange {
    Created,
    Changed(FileChangeDetails),
}

impl Report for FileChange {
    fn report(&self) {
        match self {
            FileChange::Created => println!("file created"),
            FileChange::Changed(file_change_details) => file_change_details.report(),
        }
    }
}

struct File<'a>(&'a [&'a str]);

impl File<'_> {
    fn write_to(&self, target: &mut impl io::Write) -> Result<()> {
        for line in self.0 {
            target.write_all(line.as_bytes())?;
            target.write_all(b"\n")?;
        }

        Ok(())
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

impl Report for FilePermissionChange {
    fn report(&self) {
        if let Some(mode) = self.diff.new_mode {
            println!(
                "mode: {} -> {}",
                self.existing.mode.as_oct_string(),
                mode.as_oct_string()
            );
        }
        if let Some(owner) = self.diff.new_owner {
            println!("owner: {} -> {}", self.existing.owner, owner);
        }
        if let Some(group) = self.diff.new_group {
            println!("group: {} -> {}", self.existing.group, group);
        }
    }
}

// fn file_permissions(
//     ctx: &Context,
//     path: &Path,
//     perms: FilePermissions,
// ) -> Result<Change<FilePermissionChange>> {
//     let existing = match ctx.elevate {
//         Elevate::Sudo(ref elevate_to) => {
//             let output = cmd(
//                 &["stat", "--format", "%u %g %a", "--", path.as_str()],
//                 &ctx.elevate,
//             )?;
//             let (owner, group, mode) = {
//                 let mut segments = output.stdout.trim().split(' ');
//                 let uid = Uid(segments
//                     .next()
//                     .ok_or_else(|| anyhow!("stat output could not be parsed"))?
//                     .parse::<u32>()?);
//                 let gid = Gid(segments
//                     .next()
//                     .ok_or_else(|| anyhow!("stat output could not be parsed"))?
//                     .parse::<u32>()?);
//                 let mode = FileMode(
//                     segments
//                         .next()
//                         .ok_or_else(|| anyhow!("stat output could not be parsed"))
//                         .map(|s| u32::from_str_radix(s, 8))??,
//                 );

//                 (uid, gid, mode)
//             };

//             FilePermissions { mode, owner, group }
//         }
//         Elevate::No => {
//             let stat = std::fs::metadata(path)?;

//             FilePermissions {
//                 mode: FileMode(stat.mode() & 0o6777),
//                 owner: Uid(stat.uid()),
//                 group: Gid(stat.gid()),
//             }
//         }
//     };

//     let diff = existing.diff(&perms);

//     if diff.is_empty() {
//         Ok(Change::Clean)
//     } else {
//         match ctx.elevate {
//             Elevate::Sudo(ref elevate_to) => {
//                 if let Some(uid) = diff.new_owner {
//                     cmd(
//                         &["chown", &uid.to_string(), "--", path.as_str()],
//                         &ctx.elevate,
//                     )?;
//                 }
//                 if let Some(gid) = diff.new_group {
//                     cmd(
//                         &["chgrp", &gid.to_string(), "--", path.as_str()],
//                         &ctx.elevate,
//                     )?;
//                 }
//                 if let Some(mode) = diff.new_mode {
//                     cmd(
//                         &["chmod", &mode.as_oct_string(), "--", path.as_str()],
//                         &ctx.elevate,
//                     )?;
//                 }
//             }
//             Elevate::No => {
//                 if let Some(uid) = diff.new_owner {
//                     unixfs::chown(path, Some(uid.0), None)?;
//                 }
//                 if let Some(gid) = diff.new_group {
//                     unixfs::chown(path, None, Some(gid.0))?;
//                 }
//                 if let Some(mode) = diff.new_mode {
//                     fs::set_permissions(path, fs::Permissions::from_mode(mode.0))?;
//                 }
//             }
//         }
//         Ok(Change::Changed(FilePermissionChange { existing, diff }))
//     }
// }

// fn file<'content>(
//     ctx: &Context,
//     path: &Path,
//     perms: FilePermissions,
//     content: File<'content>,
// ) -> Result<Change<FileChange>> {
//     let mut tmpfile = tempfile(ctx.cachedir)?;

//     content.write_to(&mut tmpfile)?;

//     let tmpfile_path = tmpfile.path().try_into().expect("utf-8 path");

//     let (checksum_from, checksum_to) = (
//         checksum_file(tmpfile_path)?.expect("tmpfile to exist"),
//         checksum_file(path)?,
//     );

//     match checksum_to {
//         Some(checksum_to) => {
//             let content_changed = checksum_from != checksum_to;

//             if content_changed {
//                 move_file(tmpfile_path, path, &ctx.elevate)?;
//             }

//             let permission_change = file_permissions(ctx, path, perms)?;

//             if content_changed || permission_change.is_changed() {
//                 Ok(Change::Changed(FileChange::Changed(FileChangeDetails {
//                     checksum_change: content_changed.then_some(ChecksumChange {
//                         from: checksum_from,
//                         to: checksum_to,
//                     }),
//                     permission_change: match permission_change {
//                         Change::Changed(perms) => Some(perms),
//                         Change::Clean => None,
//                     },
//                 })))
//             } else {
//                 Ok(Change::Clean)
//             }
//         }
//         None => {
//             move_file(tmpfile_path, path, &ctx.elevate)?;
//             file_permissions(ctx, path, perms)?;
//             Ok(Change::Changed(FileChange::Created))
//         }
//     }
// }

struct Plan<'a> {
    changes: Vec<Change<'a>>,
}

impl<'a> Plan<'a> {
    fn new() -> Self {
        Self { changes: vec![] }
    }

    fn add(&mut self, ctx: Box<Context<'a>>, action: Box<dyn Plannable<'a> + 'a>) -> Result<()> {
        let plan = action.plan(ctx)?;
        self.changes.push(plan);
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

impl<'a> PlanFinished<'a> {
    fn report(self) -> PlanReported<'a> {
        PlanReported {
            changes: self.changes,
        }
    }
}

struct PlanReported<'a> {
    changes: Vec<Change<'a>>,
}

impl<'a> PlanReported<'a> {
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
            match change {
                Change::Changed(plan) => {
                    plan.apply()?;
                }
                Change::Clean => {}
            }
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
        Box::new(sudo),
        Box::new(BlockInFile {
            path: Path::new("/etc/pacman.conf"),
            block: &["[multilib]", "Include = /etc/pacman.d/mirrorlist"],
            marker: "# MANAGED multilib",
            insert_after: None,
        }),
    )?;

    let plan = plan.finish();
    let plan = plan.report();
    let plan = plan.approve();

    plan.apply()?;

    // let multilib_change = block_in_file(&with_sudo)?.report("pacman multilib");

    // if multilib_change.is_changed() {
    //     pacman_refresh_package_database(&with_sudo)?.report("pacman database refresh");
    // }

    // block_in_file(
    //     &with_sudo,
    //     Path::new("/etc/pacman.conf"),
    //     &["ParallelDownloads = 5"],
    //     "# MANAGED parallel downloads",
    //     Some(Regex::new("\\[options\\]").unwrap()),
    // )?
    // .report("pacman parallel downloads");

    // file(
    //     &with_sudo,
    //     Path::new("/etc/systemd/system/pacman-cache-cleanup.service"),
    //     FilePermissions {
    //         owner: Uid::root(),
    //         group: Gid::root(),
    //         mode: FileMode(0o644),
    //     },
    //     File(&[
    //         "[Service]",
    //         "Type=oneshot",
    //         "ExecStart=/bin/sh -c '/usr/bin/paccache -rk1 && /usr/bin/paccache -ruk0'",
    //         "RemainAfterExit=true",
    //     ]),
    // )?
    // .report("pacman cache cleanup service");

    // file(
    //     &with_sudo,
    //     Path::new("/etc/systemd/system/pacman-cache-cleanup.timer"),
    //     FilePermissions {
    //         owner: Uid::root(),
    //         group: Gid::root(),
    //         mode: FileMode(0o644),
    //     },
    //     File(&[
    //         "[Timer]",
    //         "OnCalendar=daily",
    //         "",
    //         "[Install]",
    //         "WantedBy=multi-user.target",
    //     ]),
    // )?
    // .report("pacman cache cleanup timer");

    // install_packages(&with_sudo, &["pacman-contrib"])?.report("install pacman-contrib");

    Ok(())
}
