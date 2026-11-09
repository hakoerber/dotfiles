use std::{ffi::OsString, io::Write as _, os::unix::fs::PermissionsExt as _};

use anyhow::{Result, anyhow, bail};
use tokio::fs;

use super::{Elevate, Gid, MaybeOwned, Path, PathBuf, Reporter, Uid, cmd};

#[derive(Debug)]
pub(crate) struct RawFileContent(Vec<u8>);

impl RawFileContent {
    pub(crate) fn new() -> Self {
        Self(vec![])
    }

    pub(crate) fn from(from: Vec<u8>) -> Self {
        Self(from)
    }

    pub(crate) fn from_string(from: String) -> Self {
        Self(from.into_bytes())
    }

    pub(crate) fn truncate(&mut self) {
        self.0.clear();
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub(crate) fn write_all(&mut self, buf: &[u8]) {
        self.0.write_all(buf).expect("memory writes to not fail");
    }
}

#[derive(Debug)]
pub enum FileContent<'a> {
    Lines(MaybeOwned<'a, [MaybeOwned<'a, str>]>),
    Bytes(MaybeOwned<'a, [u8]>),
}

impl FileContent<'_> {
    pub(crate) fn to_bytes(&self) -> MaybeOwned<'_, [u8]> {
        match *self {
            FileContent::Lines(ref lines) => MaybeOwned::Owned({
                let mut raw = Vec::new();
                for line in lines.as_ref() {
                    raw.write_all(line.as_ref().as_bytes())
                        .expect("writing to vec always succeeds");
                    raw.write_all(b"\n")
                        .expect("writing to vec always succeeds");
                }
                raw
            }),
            FileContent::Bytes(ref bytes) => bytes.clone(),
        }
    }

    pub(crate) fn to_raw(&self) -> RawFileContent {
        let bytes = self.to_bytes();
        let owned = bytes.into_owned();
        RawFileContent(owned)
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct FileMode(pub u32);

impl FileMode {
    fn as_oct_string(self) -> String {
        format!("{0:04o}", self.0)
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct FilePermissions {
    pub owner: Uid,
    pub group: Gid,
    pub mode: FileMode,
}

#[derive(Debug)]
pub(crate) struct FilePermissionDiff {
    pub(crate) mode: Option<FileMode>,
    pub(crate) owner: Option<Uid>,
    pub(crate) group: Option<Gid>,
}

impl FilePermissionDiff {
    fn is_empty(&self) -> bool {
        self.mode.is_none() && self.owner.is_none() && self.group.is_none()
    }

    fn has_changes(&self) -> bool {
        !self.is_empty()
    }
}

impl FilePermissions {
    fn diff(&self, other: &Self) -> FilePermissionDiff {
        FilePermissionDiff {
            mode: (self.mode != other.mode).then_some(other.mode),
            owner: (self.owner != other.owner).then_some(other.owner.clone()),
            group: (self.group != other.group).then_some(other.group.clone()),
        }
    }
}

#[derive(Debug)]
pub(crate) struct FilePermissionChange {
    pub(crate) existing: FilePermissions,
    pub(crate) diff: FilePermissionDiff,
}

impl Reporter for FilePermissionChange {
    fn report(&self) -> Result<Vec<String>> {
        let mut out = vec![];
        if let Some(mode) = self.diff.mode {
            out.push(format!(
                "mode: {} -> {}",
                self.existing.mode.as_oct_string(),
                mode.as_oct_string()
            ));
        }
        if let Some(ref owner) = self.diff.owner {
            out.push(format!("owner: {} -> {}", self.existing.owner, owner));
        }
        if let Some(ref group) = self.diff.group {
            out.push(format!("group: {} -> {}", self.existing.group, group));
        }
        Ok(out)
    }
}

pub(crate) async fn mv(from: &Path, to: &Path, elevate: &Elevate) -> Result<()> {
    match *elevate {
        Elevate::Sudo(ref _elevate_to) => {
            cmd::Process::run(
                "mv",
                &[
                    "--force",
                    "--no-target-directory",
                    "--",
                    from.as_str(),
                    to.as_str(),
                ],
                elevate,
                cmd::Stdin::Closed,
            )
            .await?
            .check()?;
        }
        Elevate::No => fs::rename(from, to).await?,
    }
    Ok(())
}

pub(crate) async fn create_directory(path: &Path, elevate: &Elevate) -> Result<()> {
    match *elevate {
        Elevate::Sudo(ref _elevate_to) => {
            cmd::Process::run("mkdir", &["--", path.as_str()], elevate, cmd::Stdin::Closed)
                .await?
                .check()?;
        }
        Elevate::No => fs::create_dir(path).await?,
    }

    Ok(())
}

pub(crate) enum EntryType {
    File,
    Directory,
    Symlink,
}

pub(crate) struct Stat {
    pub(crate) entry_type: EntryType,
    pub(crate) permissions: FilePermissions,
}

// elevate is irrelevant here, always using getent. std does not support getting file names
pub(crate) async fn stat(elevate: &Elevate, path: &Path) -> Result<Option<Stat>> {
    let output = cmd::Process::run(
        "stat",
        &["--format", "%U %G %a %f", "--", path.as_str()],
        elevate,
        cmd::Stdin::Closed,
    )
    .await?
    .unchecked();

    if output.status.success() {
        let (owner, group, mode, entry_type) = {
            let mut segments = output.stdout.trim().split(' ');
            let uid = Uid(segments
                .next()
                .ok_or_else(|| anyhow!("stat output could not be parsed"))?
                .to_owned());
            let gid = Gid(segments
                .next()
                .ok_or_else(|| anyhow!("stat output could not be parsed"))?
                .to_owned());
            let mode = FileMode(
                segments
                    .next()
                    .ok_or_else(|| anyhow!("stat output could not be parsed"))
                    .map(|s| u32::from_str_radix(s, 8))??,
            );
            let entry_type = segments
                .next()
                .ok_or_else(|| anyhow!("stat output could not be parsed"))?;

            let entry_type = u16::from_str_radix(entry_type, 16)?;

            // top 4 bits indicate file type
            let entry_type = match (entry_type & 0xF000) >> 12_u16 {
                0b1100 => bail!("sockets unsupported"),
                0b1010 => EntryType::Symlink,
                0b1000 => EntryType::File,
                0b0110 => bail!("block devices unsupported"),
                0b0100 => EntryType::Directory,
                0b0010 => bail!("character devices unsupported"),
                0b0001 => bail!("fifos unsupported"),
                other => bail!("unknown file type {other}"),
            };

            (uid, gid, mode, entry_type)
        };

        Ok(Some(Stat {
            entry_type,
            permissions: FilePermissions { owner, group, mode },
        }))
    } else {
        Ok(None)
    }
}

pub(crate) async fn check_file_permissions(
    elevate: &Elevate,
    path: &Path,
    perms: &FilePermissions,
) -> Result<Option<FilePermissionChange>> {
    let existing = stat(elevate, path)
        .await?
        .ok_or_else(|| anyhow!("file not found"))?
        .permissions;

    let diff = existing.diff(perms);

    Ok(diff
        .has_changes()
        .then_some(FilePermissionChange { existing, diff }))
}

// always using chown, fs::chown does not support names
pub(crate) async fn set_file_owner(elevate: &Elevate, path: &Path, uid: &Uid) -> Result<()> {
    cmd::Process::run(
        "chown",
        &[&uid.to_string(), "--", path.as_str()],
        elevate,
        cmd::Stdin::Closed,
    )
    .await?
    .check()?;

    Ok(())
}

// always using chown, fs::chown does not support names
pub(crate) async fn set_file_group(elevate: &Elevate, path: &Path, gid: &Gid) -> Result<()> {
    cmd::Process::run(
        "chgrp",
        &[&gid.to_string(), "--", path.as_str()],
        elevate,
        cmd::Stdin::Closed,
    )
    .await?
    .check()?;
    Ok(())
}

pub(crate) async fn set_file_mode(elevate: &Elevate, path: &Path, mode: FileMode) -> Result<()> {
    match *elevate {
        Elevate::Sudo(ref _elevate_to) => {
            cmd::Process::run(
                "chmod",
                &[&mode.as_oct_string(), "--", path.as_str()],
                elevate,
                cmd::Stdin::Closed,
            )
            .await?
            .check()?;
        }
        Elevate::No => {
            fs::set_permissions(path, std::fs::Permissions::from_mode(mode.0)).await?;
        }
    }
    Ok(())
}

pub(crate) async fn remove_file(elevate: &Elevate, path: &Path) -> Result<()> {
    match *elevate {
        Elevate::Sudo(ref _elevate_to) => {
            cmd::Process::run("rm", &["--", path.as_str()], elevate, cmd::Stdin::Closed)
                .await?
                .check()?;
        }
        Elevate::No => {
            fs::remove_file(path).await?;
        }
    }
    Ok(())
}

pub(crate) fn remove_file_sync(elevate: &Elevate, path: &Path) -> Result<()> {
    match *elevate {
        Elevate::Sudo(ref _elevate_to) => {
            cmd::Process::run_sync("rm", &["--", path.as_str()], elevate, &cmd::Stdin::Closed)?
                .check()?;
        }
        Elevate::No => {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

/// `remove_file` also works for symlinks and removes only the link itself
pub(crate) async fn remove_symlink(elevate: &Elevate, path: &Path) -> Result<()> {
    remove_file(elevate, path).await
}

pub(crate) async fn remove_recursive(elevate: &Elevate, path: &Path) -> Result<()> {
    match *elevate {
        Elevate::Sudo(ref _elevate_to) => {
            cmd::Process::run(
                "rm",
                &["--recursive", "--", path.as_str()],
                elevate,
                cmd::Stdin::Closed,
            )
            .await?
            .check()?;
        }
        Elevate::No => {
            fs::remove_dir_all(path).await?;
        }
    }
    Ok(())
}

pub(crate) async fn try_read(elevate: &Elevate, path: &Path) -> Result<Option<Vec<u8>>> {
    match *elevate {
        Elevate::Sudo(ref _elevate_to) => match stat(elevate, path).await? {
            Some(_stat) => {
                let cat =
                    cmd::Process::run_bin("cat", &[path.as_str()], elevate, cmd::Stdin::Closed)
                        .await?
                        .check()?;
                Ok(Some(cat.stdout))
            }
            None => Ok(None),
        },
        Elevate::No => match fs::read(path).await {
            Ok(out) => Ok(Some(out)),
            Err(err) => {
                if err.kind() == std::io::ErrorKind::NotFound {
                    Ok(None)
                } else {
                    Err(err.into())
                }
            }
        },
    }
}

pub(crate) async fn read(elevate: &Elevate, path: &Path) -> Result<Vec<u8>> {
    try_read(elevate, path)
        .await
        .and_then(|content| content.ok_or_else(|| anyhow!("not found")))
}

pub(crate) async fn read_to_string(elevate: &Elevate, path: &Path) -> Result<String> {
    let content = read(elevate, path).await?;
    String::from_utf8(content).map_err(|_err| anyhow!("non utf-8 file content"))
}

pub(crate) async fn write(path: &Path, input: &[u8], elevate: &Elevate) -> Result<()> {
    match *elevate {
        Elevate::Sudo(ref _elevate_to) => {
            cmd::Process::run(
                "dd",
                &[&format!("of={path}")],
                elevate,
                cmd::Stdin::Write(input),
            )
            .await?
            .check()?;
        }
        Elevate::No => {
            std::fs::OpenOptions::new()
                .write(true)
                .open(path)?
                .write_all(input)?;
        }
    }
    Ok(())
}

pub async fn exists(elevate: &Elevate, path: &Path) -> Result<bool> {
    match *elevate {
        Elevate::Sudo(ref _elevate_to) => {
            Ok(
                cmd::Process::run("test", &["-d", path.as_str()], elevate, cmd::Stdin::Closed)
                    .await?
                    .unchecked()
                    .status
                    .success(),
            )
        }
        Elevate::No => Ok(fs::try_exists(path).await?),
    }
}

pub async fn files_in_dir(elevate: &Elevate, path: &Path) -> Result<Option<Vec<PathBuf>>> {
    use std::os::unix::ffi::OsStringExt as _;
    use tokio_stream::{StreamExt as _, wrappers::ReadDirStream};

    match *elevate {
        Elevate::Sudo(ref _elevate_to) => {
            if exists(elevate, path).await? {
                Ok(Some(
                    cmd::Process::run_bin(
                        "find",
                        &[
                            path.as_str(),
                            "-maxdepth",
                            "1",
                            "-type",
                            "f",
                            "-printf",
                            "%f\\0",
                        ],
                        elevate,
                        cmd::Stdin::Closed,
                    )
                    .await?
                    .check()?
                    .stdout
                    .split(|c| *c == 0)
                    .filter(|c| !c.is_empty())
                    .map(|chunk| OsString::from_vec(chunk.to_vec()))
                    .map(|s| PathBuf::try_from(s).map_err(|_err| anyhow!("non utf8 path")))
                    .collect::<Result<_>>()?,
                ))
            } else {
                Ok(None)
            }
        }
        Elevate::No => {
            match fs::read_dir(path).await {
                Ok(dir) => {
                    Ok(Some(
                        ReadDirStream::new(dir)
                            .then(|entry| async {
                                let entry = entry?;
                                let file_type = entry.file_type().await?;
                                #[expect(
                                    clippy::filetype_is_file,
                                    reason = "we are really only interested in something file-y"
                                )]
                                if file_type.is_file() || file_type.is_symlink() {
                                    Ok(Some(
                                PathBuf::try_from(
                                    entry
                                        .path()
                                        .file_name()
                                        .expect("read_dir() does not return paths with .. suffix")
                                        .to_owned(),
                                )
                                .map_err(|_err| anyhow!("non utf8 path"))?,
                            ))
                                } else {
                                    Ok(None)
                                }
                            })
                            .collect::<Result<Vec<Option<PathBuf>>>>()
                            .await?
                            .into_iter()
                            .flatten() // remove non-paths (`None`)
                            .collect(),
                    ))
                }
                Err(err) => {
                    if err.kind() == std::io::ErrorKind::NotFound {
                        Ok(None)
                    } else {
                        Err(err.into())
                    }
                }
            }
        }
    }
}
