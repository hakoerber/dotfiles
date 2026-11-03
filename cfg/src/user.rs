use anyhow::{Result, anyhow, bail};

use crate::Elevate;

use super::{PathBuf, cmd};

#[derive(Debug, Clone, derive_more::Display)]
pub(crate) struct Group {
    pub(crate) name: String,
}

#[derive(Debug)]
pub(crate) struct User {
    pub(crate) name: String,
    pub(crate) primary_group: Group,
    pub(crate) additional_groups: Vec<Group>,
    pub(crate) home: PathBuf,
    pub(crate) shell: String,
}

pub(crate) fn find_group(name: &str, elevate: &Elevate) -> Result<Option<Group>> {
    let getent = cmd::Process::run("getent", &["group", "--", name], elevate, None)?.unchecked();

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

// pub(crate) fn find_username_by_id(name: &str, elevate: &Elevate) -> Result<Option<User>> {}

pub(crate) fn find_user(name: &str, elevate: &Elevate) -> Result<Option<User>> {
    let getent = cmd::Process::run("getent", &["passwd", "--", name], elevate, None)?.unchecked();

    match getent.status.code() {
        0 => {
            let (primary, groups) =
                cmd::Process::run("id", &["-G", "--name", "--", name], elevate, None)?
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
