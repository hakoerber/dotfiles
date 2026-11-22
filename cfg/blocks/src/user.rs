use anyhow::{Result, anyhow, bail};

use crate::{Elevate, PathBuf, cmd};

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

#[derive(Debug, Clone, derive_more::Display)]
pub struct Group {
    pub name: String,
}

#[derive(Debug)]
pub struct User {
    pub name: String,
    pub primary_group: Group,
    pub additional_groups: Vec<Group>,
    pub home: PathBuf,
    pub shell: String,
}

pub async fn find_group(name: &str, elevate: &Elevate) -> Result<Option<Group>> {
    let getent = cmd::Process::run(
        "getent",
        &["group", "--", name],
        elevate,
        cmd::Stdin::Closed,
    )
    .await?
    .unchecked();

    match getent.status.code() {
        0 => {
            let [_name, _password, _gid, _members] = getent
                .stdout
                .trim()
                .split(':')
                .collect::<Vec<&str>>()
                .try_into()
                .map_err(|_err| anyhow!("invalid getent output"))?;
            Ok(Some(Group {
                name: name.to_owned(),
            }))
        }
        2 => Ok(None),
        _ => bail!("getent failed"),
    }
}

// pub fn find_username_by_id(name: &str, elevate: &Elevate) -> Result<Option<User>> {}

pub async fn find_user(name: &str, elevate: &Elevate) -> Result<Option<User>> {
    let getent = cmd::Process::run(
        "getent",
        &["passwd", "--", name],
        elevate,
        cmd::Stdin::Closed,
    )
    .await?
    .unchecked();

    match getent.status.code() {
        0 => {
            let (primary, groups) = cmd::Process::run(
                "id",
                &["-G", "--name", "--", name],
                elevate,
                cmd::Stdin::Closed,
            )
            .await?
            .check()?
            .stdout
            .split_whitespace()
            .map(|groupname| Group {
                name: groupname.to_owned(),
            })
            .enumerate()
            .fold((None, Vec::new()), |(primary, mut rest), (i, group)| {
                if i == 0 {
                    (Some(group), rest)
                } else {
                    rest.push(group);
                    (primary, rest)
                }
            });

            let primary_group = primary.ok_or_else(|| anyhow!("no primary group found"))?;

            let [
                _username,
                _password,
                _getent_uid,
                _getent_gid,
                _userinfo,
                home,
                shell,
            ] = getent
                .stdout
                .trim()
                .split(':')
                .collect::<Vec<&str>>()
                .try_into()
                .map_err(|_err| anyhow!("invalid getent output"))?;

            Ok(Some(User {
                name: name.to_owned(),
                primary_group,
                additional_groups: groups,
                home: PathBuf::from(home.to_owned()),
                shell: shell.to_owned(),
            }))
        }
        2 => Ok(None),
        _ => bail!("getent failed"),
    }
}
