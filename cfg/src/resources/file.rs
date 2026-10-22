use anyhow::{Result, anyhow, bail};

use super::super::{
    Applyable, ChangeStatus, ChecksumChange, Context, MaybeOwned, Path, Plannable, Reporter,
    Tempfile, checksum,
    fs::{self, FileContent, FilePermissionChange, FilePermissions, RawFileContent},
};

#[derive(Debug)]
#[expect(clippy::large_enum_variant, reason = "still acceptable")]
pub(crate) enum FileChange {
    Create,
    Changed {
        checksum_change: Option<ChecksumChange>,
        permission_change: Option<FilePermissionChange>,
        existing_content: RawFileContent,
    },
    ReplaceSymlink,
    ReplaceDirectory,
}

#[derive(Debug)]
pub(crate) struct FileChangePlan<'args, 'ctx> {
    pub(crate) ctx: &'ctx Context<'ctx>,
    pub(crate) args: File<'args>,
    pub(crate) change: FileChange,
}

impl Applyable for FileChangePlan<'_, '_> {
    fn apply(&self) -> Result<()> {
        fn rewrite_file(
            ctx: &Context,
            path: &Path,
            content: &MaybeOwned<'_, [u8]>,
            permissions: &FilePermissions,
        ) -> Result<()> {
            let tmpfile = Tempfile::new(&ctx.elevate)?;
            tmpfile.write(content.as_ref())?;
            tmpfile.save_as(path)?;
            fs::set_file_owner(ctx, path, &permissions.owner)?;
            fs::set_file_group(ctx, path, &permissions.group)?;
            fs::set_file_mode(ctx, path, permissions.mode)?;
            Ok(())
        }
        match self.change {
            FileChange::Create => {
                rewrite_file(
                    self.ctx,
                    self.args.path.as_ref(),
                    &self.args.content.to_bytes(),
                    &self.args.permissions,
                )?;
            }
            FileChange::ReplaceSymlink => {
                fs::remove_symlink(&self.ctx.elevate, self.args.path.as_ref())?;
                rewrite_file(
                    self.ctx,
                    self.args.path.as_ref(),
                    &self.args.content.to_bytes(),
                    &self.args.permissions,
                )?;
            }
            FileChange::ReplaceDirectory => {
                fs::remove_recursive(self.ctx, self.args.path.as_ref())?;
                rewrite_file(
                    self.ctx,
                    self.args.path.as_ref(),
                    &self.args.content.to_bytes(),
                    &self.args.permissions,
                )?;
            }
            FileChange::Changed {
                ref checksum_change,
                ref permission_change,
                existing_content: _,
            } => match *checksum_change {
                Some(ref _change) => {
                    rewrite_file(
                        self.ctx,
                        self.args.path.as_ref(),
                        &self.args.content.to_bytes(),
                        &self.args.permissions,
                    )?;
                }
                None => {
                    if let Some(ref permission_change) = *permission_change {
                        if let Some(ref uid) = permission_change.diff.owner {
                            fs::set_file_owner(self.ctx, self.args.path.as_ref(), uid)?;
                        }
                        if let Some(ref gid) = permission_change.diff.group {
                            fs::set_file_group(self.ctx, self.args.path.as_ref(), gid)?;
                        }
                        if let Some(mode) = permission_change.diff.mode {
                            fs::set_file_mode(self.ctx, self.args.path.as_ref(), mode)?;
                        }
                    }
                }
            },
        }
        Ok(())
    }
}

impl Reporter for FileChangePlan<'_, '_> {
    fn report(&self) -> Result<Vec<String>> {
        match self.change {
            FileChange::Create => {
                let mut out = vec!["create file".to_owned()];

                let new_content = self.args.content.to_raw();

                if !self.args.binary {
                    let diff = diffy::create_patch_bytes(&[], new_content.as_bytes());

                    let formatter = diffy::PatchFormatter::new().with_color();
                    let mut buffer = Vec::<u8>::new();
                    formatter.write_patch_into(&diff, &mut buffer)?;

                    let diff =
                        String::from_utf8(buffer).map_err(|_err| anyhow!("non utf-8 patch"))?;
                    out.extend(diff.lines().map(|s| s.to_owned()).collect::<Vec<String>>());
                }

                Ok(out)
            }
            FileChange::ReplaceSymlink => Ok(vec!["replacing symlink".to_owned()]),
            FileChange::ReplaceDirectory => Ok(vec!["replacing directory".to_owned()]),
            FileChange::Changed {
                ref existing_content,
                ref checksum_change,
                ref permission_change,
            } => {
                let mut out = vec![];

                if let Some(ref permission_change) = *permission_change {
                    out.extend(permission_change.report()?);
                }

                if let Some(ref checksum_change) = *checksum_change {
                    out.push(format!(
                        "checksum change: {} -> {}",
                        checksum_change.from, checksum_change.to
                    ));

                    let new_content = self.args.content.to_raw();

                    let diff = diffy::create_patch_bytes(
                        existing_content.as_bytes(),
                        new_content.as_bytes(),
                    );

                    let formatter = diffy::PatchFormatter::new().with_color();
                    let mut buffer = Vec::<u8>::new();
                    formatter.write_patch_into(&diff, &mut buffer)?;

                    let diff =
                        String::from_utf8(buffer).map_err(|_err| anyhow!("non utf-8 patch"))?;
                    out.extend(diff.lines().map(|s| s.to_owned()).collect::<Vec<String>>());
                }

                Ok(out)
            }
        }
    }
}

#[derive(Debug)]
pub struct File<'args> {
    pub path: MaybeOwned<'args, Path>,
    pub permissions: FilePermissions,
    pub on_symlink: SymlinkHandling,
    pub on_directory: DirectoryHandling,
    pub content: FileContent<'args>,
    pub binary: bool,
}

impl<'args> Plannable<'args> for File<'args> {
    fn plan<'ctx>(self: Box<Self>, ctx: &'ctx Context<'ctx>) -> Result<ChangeStatus<'args>>
    where
        'ctx: 'args,
    {
        let stat = fs::stat(&ctx.elevate, self.path.as_ref())?;

        if let Some(stat) = stat {
            match stat.entry_type {
                fs::EntryType::File => {
                    let existing_content = fs::read(&ctx.elevate, self.path.as_ref())?;

                    let (checksum_from, checksum_to) = (
                        checksum::checksum_from(self.content.to_bytes().as_ref()),
                        checksum::checksum_from(&existing_content),
                    );

                    let content_changed = checksum_from != checksum_to;

                    let permission_change =
                        fs::check_file_permissions(ctx, self.path.as_ref(), &self.permissions)?;

                    if content_changed || permission_change.is_some() {
                        Ok(ChangeStatus::NeedsChange(Box::new(FileChangePlan {
                            ctx,
                            args: *self,
                            change: FileChange::Changed {
                                existing_content: RawFileContent::from(existing_content),
                                checksum_change: content_changed.then_some(ChecksumChange {
                                    from: checksum_from,
                                    to: checksum_to,
                                }),
                                permission_change,
                            },
                        })))
                    } else {
                        Ok(ChangeStatus::Clean)
                    }
                }
                fs::EntryType::Directory => match self.on_directory {
                    DirectoryHandling::Bail => bail!("refusing to overwrite directory"),
                    DirectoryHandling::Proceed => {
                        Ok(ChangeStatus::NeedsChange(Box::new(FileChangePlan {
                            ctx,
                            args: *self,
                            change: FileChange::ReplaceDirectory,
                        })))
                    }
                },
                fs::EntryType::Symlink => match self.on_symlink {
                    SymlinkHandling::Proceed => {
                        Ok(ChangeStatus::NeedsChange(Box::new(FileChangePlan {
                            ctx,
                            args: *self,
                            change: FileChange::ReplaceSymlink,
                        })))
                    }
                    SymlinkHandling::Bail => bail!("refusing to overwrite symlink"),
                },
            }
        } else {
            // not found
            Ok(ChangeStatus::NeedsChange(Box::new(FileChangePlan {
                ctx,
                args: *self,
                change: FileChange::Create,
            })))
        }
    }
}

#[derive(Debug)]
pub(crate) enum DirectoryChange {
    Create,
    ReplaceSymlink,
    ReplaceFile,
    Changed {
        permission_change: Option<FilePermissionChange>,
    },
}

#[derive(Debug)]
pub(crate) struct DirectoryChangePlan<'args, 'ctx> {
    pub(crate) ctx: &'ctx Context<'ctx>,
    pub(crate) args: Directory<'args>,
    pub(crate) change: DirectoryChange,
}

impl Applyable for DirectoryChangePlan<'_, '_> {
    fn apply(&self) -> Result<()> {
        fn create(path: &Path, permissions: &FilePermissions, ctx: &Context) -> Result<()> {
            fs::create_directory(path, &ctx.elevate)?;
            fs::set_file_owner(ctx, path, &permissions.owner)?;
            fs::set_file_group(ctx, path, &permissions.group)?;
            fs::set_file_mode(ctx, path, permissions.mode)?;
            Ok(())
        }

        match self.change {
            DirectoryChange::Create => {
                create(self.args.path.as_ref(), &self.args.permissions, self.ctx)?;
            }
            DirectoryChange::ReplaceSymlink => {
                fs::remove_symlink(&self.ctx.elevate, self.args.path.as_ref())?;
                create(self.args.path.as_ref(), &self.args.permissions, self.ctx)?;
            }
            DirectoryChange::ReplaceFile => {
                fs::remove_file(&self.ctx.elevate, self.args.path.as_ref())?;
                create(self.args.path.as_ref(), &self.args.permissions, self.ctx)?;
            }
            DirectoryChange::Changed {
                ref permission_change,
            } => {
                if let Some(ref permission_change) = *permission_change {
                    if let Some(ref uid) = permission_change.diff.owner {
                        fs::set_file_owner(self.ctx, self.args.path.as_ref(), uid)?;
                    }
                    if let Some(ref gid) = permission_change.diff.group {
                        fs::set_file_group(self.ctx, self.args.path.as_ref(), gid)?;
                    }
                    if let Some(mode) = permission_change.diff.mode {
                        fs::set_file_mode(self.ctx, self.args.path.as_ref(), mode)?;
                    }
                }
            }
        }
        Ok(())
    }
}

impl Reporter for DirectoryChangePlan<'_, '_> {
    fn report(&self) -> Result<Vec<String>> {
        match self.change {
            DirectoryChange::Create => Ok(vec!["create file".to_owned()]),
            DirectoryChange::Changed {
                ref permission_change,
            } => {
                let mut out = vec![];

                if let Some(ref permission_change) = *permission_change {
                    out.extend(permission_change.report()?);
                }

                Ok(out)
            }
            DirectoryChange::ReplaceSymlink => Ok(vec!["replace symlink".to_owned()]),
            DirectoryChange::ReplaceFile => Ok(vec!["replace file".to_owned()]),
        }
    }
}

#[derive(Debug)]
pub enum SymlinkHandling {
    Bail,
    Proceed,
}

#[derive(Debug)]
pub enum FileHandling {
    Bail,
    Proceed,
}

#[derive(Debug)]
pub enum DirectoryHandling {
    Bail,
    Proceed,
}

#[derive(Debug)]
pub struct Directory<'args> {
    pub path: MaybeOwned<'args, Path>,
    pub on_symlink: SymlinkHandling,
    pub on_file: FileHandling,
    pub permissions: FilePermissions,
}

impl<'args> Plannable<'args> for Directory<'args> {
    fn plan<'ctx>(self: Box<Self>, ctx: &'ctx Context<'ctx>) -> Result<ChangeStatus<'args>>
    where
        'ctx: 'args,
    {
        let stat = fs::stat(&ctx.elevate, self.path.as_ref())?;

        if let Some(stat) = stat {
            match stat.entry_type {
                fs::EntryType::File => match self.on_file {
                    FileHandling::Proceed => {
                        Ok(ChangeStatus::NeedsChange(Box::new(DirectoryChangePlan {
                            ctx,
                            args: *self,
                            change: DirectoryChange::ReplaceFile,
                        })))
                    }
                    FileHandling::Bail => bail!("refusing to overwrite file"),
                },
                fs::EntryType::Symlink => match self.on_symlink {
                    SymlinkHandling::Proceed => {
                        Ok(ChangeStatus::NeedsChange(Box::new(DirectoryChangePlan {
                            ctx,
                            args: *self,
                            change: DirectoryChange::ReplaceSymlink,
                        })))
                    }
                    SymlinkHandling::Bail => bail!("refusing to overwrite symlink"),
                },
                fs::EntryType::Directory => {
                    let permission_change =
                        fs::check_file_permissions(ctx, self.path.as_ref(), &self.permissions)?;

                    if permission_change.is_some() {
                        Ok(ChangeStatus::NeedsChange(Box::new(DirectoryChangePlan {
                            ctx,
                            args: *self,
                            change: DirectoryChange::Changed { permission_change },
                        })))
                    } else {
                        Ok(ChangeStatus::Clean)
                    }
                }
            }
        } else {
            // does not exist
            Ok(ChangeStatus::NeedsChange(Box::new(DirectoryChangePlan {
                ctx,
                args: *self,
                change: DirectoryChange::Create,
            })))
        }
    }
}

#[derive(Debug)]
pub(crate) enum RemoveChange {
    Directory,
    File,
    Symlink,
}

#[derive(Debug)]
pub(crate) struct RemovePlan<'args, 'ctx> {
    pub(crate) ctx: &'ctx Context<'ctx>,
    pub(crate) args: Remove<'args>,
    pub(crate) change: RemoveChange,
}

impl Applyable for RemovePlan<'_, '_> {
    fn apply(&self) -> Result<()> {
        match self.change {
            RemoveChange::Directory => fs::remove_recursive(self.ctx, self.args.path.as_ref()),
            RemoveChange::File => fs::remove_file(&self.ctx.elevate, self.args.path.as_ref()),
            RemoveChange::Symlink => fs::remove_symlink(&self.ctx.elevate, self.args.path.as_ref()),
        }
    }
}

impl Reporter for RemovePlan<'_, '_> {
    fn report(&self) -> Result<Vec<String>> {
        match self.change {
            RemoveChange::Directory => Ok(vec!["remove directory".to_owned()]),
            RemoveChange::File => Ok(vec!["remove file".to_owned()]),
            RemoveChange::Symlink => Ok(vec!["remove symlink".to_owned()]),
        }
    }
}

#[derive(Debug)]
pub struct Remove<'args> {
    pub path: MaybeOwned<'args, Path>,
    pub on_directory: DirectoryHandling,
    pub on_file: FileHandling,
    pub on_symlink: SymlinkHandling,
}

impl<'args> Plannable<'args> for Remove<'args> {
    fn plan<'ctx>(self: Box<Self>, ctx: &'ctx Context<'ctx>) -> Result<ChangeStatus<'args>>
    where
        'ctx: 'args,
    {
        match fs::stat(&ctx.elevate, self.path.as_ref())? {
            Some(stat) => Ok(ChangeStatus::NeedsChange(Box::new(RemovePlan {
                change: match stat.entry_type {
                    fs::EntryType::File => match self.on_file {
                        FileHandling::Bail => bail!("refusing to remove file"),
                        FileHandling::Proceed => RemoveChange::File,
                    },

                    fs::EntryType::Directory => match self.on_directory {
                        DirectoryHandling::Bail => bail!("refusing to remove directory"),
                        DirectoryHandling::Proceed => RemoveChange::Directory,
                    },
                    fs::EntryType::Symlink => match self.on_symlink {
                        SymlinkHandling::Bail => bail!("refusing to remove symlink"),
                        SymlinkHandling::Proceed => RemoveChange::Symlink,
                    },
                },
                ctx,
                args: *self,
            }))),
            None => Ok(ChangeStatus::Clean),
        }
    }
}
