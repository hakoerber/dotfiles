use std::{
    env, fmt, fs,
    io::{self, Read, Write},
    os::unix::fs::{self as unixfs, MetadataExt as _, PermissionsExt},
    process::{self},
};

use anyhow::{Result, anyhow};

use super::{
    Applyable, ChangeStatus, ChecksumChange, Context, Elevate, FileContent, FileMode,
    FilePermissionChange, FilePermissions, Gid, Path, Plannable, RawFileContent, Reporter,
    Tempfile, Uid, checksum, cmd,
};

pub(crate) fn mv(from: &Path, to: &Path, elevate: &Elevate) -> Result<()> {
    match elevate {
        Elevate::Sudo(elevate_to) => {
            cmd::cmd(
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

fn check_file_permissions(
    ctx: &Context,
    path: &Path,
    perms: &FilePermissions,
) -> Result<Option<FilePermissionChange>> {
    let existing = match ctx.elevate {
        Elevate::Sudo(ref elevate_to) => {
            let output = cmd::cmd(
                &["stat", "--format", "%u %g %a", "--", path.as_str()],
                &ctx.elevate,
            )?;
            let (owner, group, mode) = {
                let mut segments = output.stdout.trim().split(' ');
                let uid = Uid(segments
                    .next()
                    .ok_or_else(|| anyhow!("stat output could not be parsed"))?
                    .parse::<u32>()?);
                let gid = Gid(segments
                    .next()
                    .ok_or_else(|| anyhow!("stat output could not be parsed"))?
                    .parse::<u32>()?);
                let mode = FileMode(
                    segments
                        .next()
                        .ok_or_else(|| anyhow!("stat output could not be parsed"))
                        .map(|s| u32::from_str_radix(s, 8))??,
                );

                (uid, gid, mode)
            };

            FilePermissions { mode, owner, group }
        }
        Elevate::No => {
            let stat = std::fs::metadata(path)?;

            FilePermissions {
                mode: FileMode(stat.mode() & 0o6777),
                owner: Uid(stat.uid()),
                group: Gid(stat.gid()),
            }
        }
    };

    let diff = existing.diff(&perms);

    Ok(diff
        .has_changes()
        .then_some(FilePermissionChange { existing, diff }))
}

fn set_file_owner(ctx: &Context, path: &Path, uid: Uid) -> Result<()> {
    match ctx.elevate {
        Elevate::Sudo(ref elevate_to) => {
            cmd::cmd(
                &["chown", &uid.to_string(), "--", path.as_str()],
                &ctx.elevate,
            )?;
            Ok(())
        }
        Elevate::No => {
            unixfs::chown(path, Some(uid.0), None)?;
            Ok(())
        }
    }
}

fn set_file_group(ctx: &Context, path: &Path, gid: Gid) -> Result<()> {
    match ctx.elevate {
        Elevate::Sudo(ref elevate_to) => {
            cmd::cmd(
                &["chgrp", &gid.to_string(), "--", path.as_str()],
                &ctx.elevate,
            )?;
            Ok(())
        }
        Elevate::No => {
            unixfs::chown(path, None, Some(gid.0))?;
            Ok(())
        }
    }
}

fn set_file_mode(ctx: &Context, path: &Path, mode: FileMode) -> Result<()> {
    match ctx.elevate {
        Elevate::Sudo(ref elevate_to) => {
            cmd::cmd(
                &["chmod", &mode.as_oct_string(), "--", path.as_str()],
                &ctx.elevate,
            )?;
            Ok(())
        }
        Elevate::No => {
            fs::set_permissions(path, fs::Permissions::from_mode(mode.0))?;
            Ok(())
        }
    }
}

#[derive(Debug)]
pub(crate) enum FileChange {
    Create,
    Changed {
        checksum_change: Option<ChecksumChange>,
        permission_change: Option<FilePermissionChange>,
    },
}

#[derive(Debug)]
pub(crate) struct FileChangePlan<'a> {
    pub(crate) ctx: &'a Context<'a>,
    pub(crate) args: File<'a>,
    pub(crate) new_content: RawFileContent,
    pub(crate) change: FileChange,
}

impl Applyable for FileChangePlan<'_> {
    fn apply(&self) -> Result<()> {
        fn rewrite_file(
            ctx: &Context,
            path: &Path,
            content: &RawFileContent,
            permissions: &FilePermissions,
        ) -> Result<()> {
            let mut tmpfile = Tempfile::new_in(ctx.cachedir)?;
            tmpfile.write_all(content.as_bytes())?;
            mv(tmpfile.path(), path, &ctx.elevate)?;
            set_file_owner(ctx, path, permissions.owner)?;
            set_file_group(ctx, path, permissions.group)?;
            set_file_mode(ctx, path, permissions.mode)?;
            Ok(())
        }
        match self.change {
            FileChange::Create => {
                rewrite_file(
                    self.ctx,
                    self.args.path,
                    &self.new_content,
                    &self.args.permissions,
                )?;
                Ok(())
            }
            FileChange::Changed {
                ref checksum_change,
                ref permission_change,
            } => match *checksum_change {
                Some(ref change) => {
                    rewrite_file(
                        self.ctx,
                        self.args.path,
                        &self.new_content,
                        &self.args.permissions,
                    )?;
                    Ok(())
                }
                None => match *permission_change {
                    Some(ref permission_change) => {
                        if let Some(uid) = permission_change.diff.new_owner {
                            set_file_owner(self.ctx, self.args.path, uid)?;
                        }
                        if let Some(gid) = permission_change.diff.new_group {
                            set_file_group(self.ctx, self.args.path, gid)?;
                        }
                        if let Some(mode) = permission_change.diff.new_mode {
                            set_file_mode(self.ctx, self.args.path, mode)?;
                        }

                        Ok(())
                    }
                    None => Ok(()),
                },
            },
        }
    }
}

impl Reporter for FileChangePlan<'_> {
    fn report(&self) -> Vec<String> {
        match self.change {
            FileChange::Create => vec!["create file".to_owned()],
            FileChange::Changed {
                ref checksum_change,
                ref permission_change,
            } => {
                let mut out = vec![];
                if let Some(checksum_change) = checksum_change {
                    out.push(format!(
                        "checksum change: {} -> {}",
                        checksum_change.from, checksum_change.to
                    ));
                };
                if let Some(permission_change) = permission_change {
                    out.extend(permission_change.report())
                };
                out
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct File<'a> {
    pub(crate) path: &'a Path,
    pub(crate) permissions: FilePermissions,
    pub(crate) content: FileContent<'a>,
}

impl<'a> Plannable<'a> for File<'a> {
    fn plan(self: Box<Self>, ctx: &'a Context<'a>) -> Result<ChangeStatus<'a>> {
        let mut new_content = self.content.as_raw();

        let (checksum_from, checksum_to) = (
            checksum::checksum_from(self.content.as_raw().as_bytes()),
            checksum::checksum_file(self.path)?,
        );

        match checksum_to {
            Some(checksum_to) => {
                let content_changed = checksum_from != checksum_to;

                let permission_change = check_file_permissions(ctx, self.path, &self.permissions)?;

                if content_changed || permission_change.is_some() {
                    Ok(ChangeStatus::NeedsChange(Box::new(FileChangePlan {
                        ctx,
                        args: *self,
                        new_content,
                        change: FileChange::Changed {
                            checksum_change: content_changed.then_some(ChecksumChange {
                                from: checksum_from,
                                to: checksum_to,
                            }),
                            permission_change: match permission_change {
                                Some(perms) => Some(perms),
                                None => None,
                            },
                        },
                    })))
                } else {
                    Ok(ChangeStatus::Clean)
                }
            }
            None => {
                // source file does not exist
                Ok(ChangeStatus::NeedsChange(Box::new(FileChangePlan {
                    ctx,
                    args: *self,
                    new_content,
                    change: FileChange::Create,
                })))
            }
        }
    }
}
