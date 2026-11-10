use anyhow::{Result, anyhow, bail};
use async_trait::async_trait;

use super::super::{
    Applyable, Assertable, AssertionFailureReport, AssertionResult, ChangeStatus, ChecksumChange,
    Elevate, MaybeOwned, Path, Plannable, Reporter, Tempfile, checksum,
    fs::{self, FileContent, FilePermissionChange, FilePermissions, RawFileContent},
};

#[derive(Debug)]
pub(crate) enum FileChange {
    Create,
    MetadataChange {
        permission_change: FilePermissionChange,
    },
    Changed {
        checksum_change: Option<ChecksumChange>,
        permission_change: Option<FilePermissionChange>,
        existing_content: RawFileContent,
    },
    ReplaceSymlink,
    ReplaceDirectory,
}

#[derive(Debug)]
pub(crate) struct FileChangePlan<'args> {
    pub(crate) args: File<'args>,
    pub(crate) change: FileChange,
}

#[async_trait]
impl Applyable for FileChangePlan<'_> {
    async fn apply(&self, elevate: &Elevate) -> Result<()> {
        async fn rewrite_file(
            elevate: &Elevate,
            path: &Path,
            content: &MaybeOwned<'_, [u8]>,
            permissions: &FilePermissions,
        ) -> Result<()> {
            let tmpfile = Tempfile::new(elevate).await?;
            tmpfile.write(content.as_ref()).await?;
            // clippy::large_futures
            Box::pin(tmpfile.save_as(path)).await?;
            fs::set_file_owner(elevate, path, &permissions.owner).await?;
            fs::set_file_group(elevate, path, &permissions.group).await?;
            fs::set_file_mode(elevate, path, permissions.mode).await?;
            Ok(())
        }
        match self.change {
            FileChange::Create => {
                Box::pin(rewrite_file(
                    elevate,
                    self.args.path.as_ref(),
                    &self.args.content.to_bytes(),
                    &self.args.permissions,
                ))
                .await?;
            }
            FileChange::ReplaceSymlink => {
                fs::remove_symlink(elevate, self.args.path.as_ref()).await?;
                Box::pin(rewrite_file(
                    elevate,
                    self.args.path.as_ref(),
                    &self.args.content.to_bytes(),
                    &self.args.permissions,
                ))
                .await?;
            }
            FileChange::ReplaceDirectory => {
                fs::remove_recursive(elevate, self.args.path.as_ref()).await?;
                Box::pin(rewrite_file(
                    elevate,
                    self.args.path.as_ref(),
                    &self.args.content.to_bytes(),
                    &self.args.permissions,
                ))
                .await?;
            }
            FileChange::MetadataChange {
                ref permission_change,
            } => {
                if let Some(ref uid) = permission_change.diff.owner {
                    fs::set_file_owner(elevate, self.args.path.as_ref(), uid).await?;
                }
                if let Some(ref gid) = permission_change.diff.group {
                    fs::set_file_group(elevate, self.args.path.as_ref(), gid).await?;
                }
                if let Some(mode) = permission_change.diff.mode {
                    fs::set_file_mode(elevate, self.args.path.as_ref(), mode).await?;
                }
            }
            FileChange::Changed {
                ref checksum_change,
                ref permission_change,
                existing_content: _,
            } => match *checksum_change {
                Some(ref _change) => {
                    Box::pin(rewrite_file(
                        elevate,
                        self.args.path.as_ref(),
                        &self.args.content.to_bytes(),
                        &self.args.permissions,
                    ))
                    .await?;
                }
                None => {
                    if let Some(ref permission_change) = *permission_change {
                        if let Some(ref uid) = permission_change.diff.owner {
                            fs::set_file_owner(elevate, self.args.path.as_ref(), uid).await?;
                        }
                        if let Some(ref gid) = permission_change.diff.group {
                            fs::set_file_group(elevate, self.args.path.as_ref(), gid).await?;
                        }
                        if let Some(mode) = permission_change.diff.mode {
                            fs::set_file_mode(elevate, self.args.path.as_ref(), mode).await?;
                        }
                    }
                }
            },
        }
        Ok(())
    }
}

impl Reporter for FileChangePlan<'_> {
    fn report(&self) -> Result<Vec<String>> {
        match self.change {
            FileChange::Create => {
                let mut out = vec!["create file".to_owned()];

                let new_content = self.args.content.to_raw();

                if self.args.secret {
                    out.push("diff suppressed due to secrets".to_owned());
                } else if !self.args.binary {
                    let diff = diffy::create_patch_bytes(&[], new_content.as_bytes());

                    let formatter = diffy::PatchFormatter::new().with_color();
                    let mut buffer = Vec::<u8>::new();
                    formatter.write_patch_into(&diff, &mut buffer)?;

                    let diff = String::from_utf8(buffer).map_err(|_err| {
                        anyhow!("non utf-8 patch in {}", self.args.path.as_ref())
                    })?;
                    out.extend(diff.lines().map(|s| s.to_owned()).collect::<Vec<String>>());
                }

                Ok(out)
            }
            FileChange::ReplaceSymlink => Ok(vec!["replacing symlink".to_owned()]),
            FileChange::ReplaceDirectory => Ok(vec!["replacing directory".to_owned()]),
            FileChange::MetadataChange {
                ref permission_change,
            } => Ok(permission_change.report()?),
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

                    if self.args.secret {
                        out.push("diff suppressed due to secrets".to_owned());
                    } else if !self.args.binary {
                        let diff = diffy::create_patch_bytes(
                            existing_content.as_bytes(),
                            new_content.as_bytes(),
                        );

                        let formatter = diffy::PatchFormatter::new().with_color();
                        let mut buffer = Vec::<u8>::new();
                        formatter.write_patch_into(&diff, &mut buffer)?;

                        let diff = String::from_utf8(buffer).map_err(|_err| {
                            anyhow!("non utf-8 patch in {}", self.args.path.as_ref())
                        })?;
                        out.extend(diff.lines().map(|s| s.to_owned()).collect::<Vec<String>>());
                    }
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
    pub secret: bool,
    pub binary: bool,
    pub ignore_checksum_changes: bool,
}

#[async_trait]
impl<'args> Plannable<'args> for File<'args> {
    async fn plan<'elevate>(
        self: Box<Self>,
        elevate: &'elevate Elevate,
    ) -> Result<ChangeStatus<'args>>
    where
        'elevate: 'args,
    {
        let stat = fs::stat(elevate, self.path.as_ref()).await?;

        if let Some(stat) = stat {
            match stat.entry_type {
                fs::EntryType::File => {
                    let permission_change =
                        fs::check_file_permissions(elevate, self.path.as_ref(), &self.permissions)
                            .await?;

                    if self.ignore_checksum_changes {
                        if let Some(permission_change) = permission_change {
                            Ok(ChangeStatus::NeedsChange(Box::new(FileChangePlan {
                                args: *self,
                                change: FileChange::MetadataChange { permission_change },
                            })))
                        } else {
                            Ok(ChangeStatus::Clean)
                        }
                    } else {
                        let existing_content = fs::read(elevate, self.path.as_ref()).await?;

                        let (checksum_from, checksum_to) = (
                            checksum::checksum_from(self.content.to_bytes().as_ref()),
                            checksum::checksum_from(&existing_content),
                        );

                        let content_changed = checksum_from != checksum_to;

                        if content_changed || permission_change.is_some() {
                            Ok(ChangeStatus::NeedsChange(Box::new(FileChangePlan {
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
                }
                fs::EntryType::Directory => match self.on_directory {
                    DirectoryHandling::Bail => bail!("refusing to overwrite directory"),
                    DirectoryHandling::Proceed => {
                        Ok(ChangeStatus::NeedsChange(Box::new(FileChangePlan {
                            args: *self,
                            change: FileChange::ReplaceDirectory,
                        })))
                    }
                },
                fs::EntryType::Symlink => match self.on_symlink {
                    SymlinkHandling::Proceed => {
                        Ok(ChangeStatus::NeedsChange(Box::new(FileChangePlan {
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
pub(crate) struct DirectoryChangePlan<'args> {
    pub(crate) args: Directory<'args>,
    pub(crate) change: DirectoryChange,
}

#[async_trait]
impl Applyable for DirectoryChangePlan<'_> {
    async fn apply(&self, elevate: &Elevate) -> Result<()> {
        async fn create(
            path: &Path,
            permissions: &FilePermissions,
            elevate: &Elevate,
        ) -> Result<()> {
            fs::create_directory(path, elevate).await?;
            fs::set_file_owner(elevate, path, &permissions.owner).await?;
            fs::set_file_group(elevate, path, &permissions.group).await?;
            fs::set_file_mode(elevate, path, permissions.mode).await?;
            Ok(())
        }

        match self.change {
            DirectoryChange::Create => {
                Box::pin(create(
                    self.args.path.as_ref(),
                    &self.args.permissions,
                    elevate,
                ))
                .await?;
            }
            DirectoryChange::ReplaceSymlink => {
                fs::remove_symlink(elevate, self.args.path.as_ref()).await?;
                Box::pin(create(
                    self.args.path.as_ref(),
                    &self.args.permissions,
                    elevate,
                ))
                .await?;
            }
            DirectoryChange::ReplaceFile => {
                fs::remove_file(elevate, self.args.path.as_ref()).await?;
                Box::pin(create(
                    self.args.path.as_ref(),
                    &self.args.permissions,
                    elevate,
                ))
                .await?;
            }
            DirectoryChange::Changed {
                ref permission_change,
            } => {
                if let Some(ref permission_change) = *permission_change {
                    if let Some(ref uid) = permission_change.diff.owner {
                        fs::set_file_owner(elevate, self.args.path.as_ref(), uid).await?;
                    }
                    if let Some(ref gid) = permission_change.diff.group {
                        fs::set_file_group(elevate, self.args.path.as_ref(), gid).await?;
                    }
                    if let Some(mode) = permission_change.diff.mode {
                        fs::set_file_mode(elevate, self.args.path.as_ref(), mode).await?;
                    }
                }
            }
        }
        Ok(())
    }
}

impl Reporter for DirectoryChangePlan<'_> {
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

#[async_trait]
impl<'args> Plannable<'args> for Directory<'args> {
    async fn plan<'elevate>(
        self: Box<Self>,
        elevate: &'elevate Elevate,
    ) -> Result<ChangeStatus<'args>>
    where
        'elevate: 'args,
    {
        let stat = fs::stat(elevate, self.path.as_ref()).await?;

        if let Some(stat) = stat {
            match stat.entry_type {
                fs::EntryType::File => match self.on_file {
                    FileHandling::Proceed => {
                        Ok(ChangeStatus::NeedsChange(Box::new(DirectoryChangePlan {
                            args: *self,
                            change: DirectoryChange::ReplaceFile,
                        })))
                    }
                    FileHandling::Bail => bail!("refusing to overwrite file"),
                },
                fs::EntryType::Symlink => match self.on_symlink {
                    SymlinkHandling::Proceed => {
                        Ok(ChangeStatus::NeedsChange(Box::new(DirectoryChangePlan {
                            args: *self,
                            change: DirectoryChange::ReplaceSymlink,
                        })))
                    }
                    SymlinkHandling::Bail => bail!("refusing to overwrite symlink"),
                },
                fs::EntryType::Directory => {
                    let permission_change =
                        fs::check_file_permissions(elevate, self.path.as_ref(), &self.permissions)
                            .await?;

                    if permission_change.is_some() {
                        Ok(ChangeStatus::NeedsChange(Box::new(DirectoryChangePlan {
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
pub(crate) struct RemovePlan<'args> {
    pub(crate) args: Remove<'args>,
    pub(crate) change: RemoveChange,
}

#[async_trait]
impl Applyable for RemovePlan<'_> {
    async fn apply(&self, elevate: &Elevate) -> Result<()> {
        match self.change {
            RemoveChange::Directory => fs::remove_recursive(elevate, self.args.path.as_ref()).await,
            RemoveChange::File => fs::remove_file(elevate, self.args.path.as_ref()).await,
            RemoveChange::Symlink => fs::remove_symlink(elevate, self.args.path.as_ref()).await,
        }
    }
}

impl Reporter for RemovePlan<'_> {
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

#[async_trait]
impl<'args> Plannable<'args> for Remove<'args> {
    async fn plan<'elevate>(
        self: Box<Self>,
        elevate: &'elevate Elevate,
    ) -> Result<ChangeStatus<'args>>
    where
        'elevate: 'args,
    {
        match fs::stat(elevate, self.path.as_ref()).await? {
            Some(stat) => Ok(ChangeStatus::NeedsChange(Box::new(RemovePlan {
                change: match stat.entry_type {
                    fs::EntryType::File => match self.on_file {
                        FileHandling::Bail => {
                            bail!("refusing to remove file {}", self.path.as_ref())
                        }
                        FileHandling::Proceed => RemoveChange::File,
                    },

                    fs::EntryType::Directory => match self.on_directory {
                        DirectoryHandling::Bail => {
                            bail!("refusing to remove directory {}", self.path.as_ref())
                        }
                        DirectoryHandling::Proceed => RemoveChange::Directory,
                    },
                    fs::EntryType::Symlink => match self.on_symlink {
                        SymlinkHandling::Bail => {
                            bail!("refusing to remove symlink {}", self.path.as_ref())
                        }
                        SymlinkHandling::Proceed => RemoveChange::Symlink,
                    },
                },
                args: *self,
            }))),
            None => Ok(ChangeStatus::Clean),
        }
    }
}

#[derive(Debug)]
pub struct AssertFile<'args> {
    pub path: MaybeOwned<'args, Path>,
    pub permissions: FilePermissions,
}

#[async_trait]
impl Assertable<'_> for AssertFile<'_> {
    async fn assert<'elevate>(
        self: Box<Self>,
        elevate: &'elevate Elevate,
    ) -> Result<AssertionResult> {
        let stat = fs::stat(elevate, self.path.as_ref()).await?;

        if let Some(stat) = stat {
            match stat.entry_type {
                fs::EntryType::File => {
                    let permission_change =
                        fs::check_file_permissions(elevate, self.path.as_ref(), &self.permissions)
                            .await?;

                    if let Some(permission_change) = permission_change {
                        Ok(AssertionResult::Failed(AssertionFailureReport {
                            reason: {
                                let mut out = vec!["permission mismatch".to_owned()];
                                out.extend(permission_change.report()?);
                                out
                            },
                        }))
                    } else {
                        Ok(AssertionResult::Success)
                    }
                }
                fs::EntryType::Directory => Ok(AssertionResult::Failed(AssertionFailureReport {
                    reason: vec![format!(
                        "{} is a directory (expected file)",
                        self.path.as_ref()
                    )],
                })),
                fs::EntryType::Symlink => Ok(AssertionResult::Failed(AssertionFailureReport {
                    reason: vec![format!(
                        "{} is a symlink (expected file)",
                        self.path.as_ref()
                    )],
                })),
            }
        } else {
            Ok(AssertionResult::Failed(AssertionFailureReport {
                reason: vec![format!("file {} does not exist", self.path.as_ref())],
            }))
        }
    }
}
