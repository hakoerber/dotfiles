use anyhow::Result;

use crate::{maybeowned::MaybeOwned, user};

use super::super::{Applyable, ChangeStatus, Context, Path, Plannable, Reporter, cmd};

#[derive(Debug)]
pub(crate) enum UserGroupDiff<'change> {
    Add(&'change str),
    Remove(user::Group),
}

#[derive(Debug)]
pub(crate) struct UserDiff<'change> {
    new_primary_group: Option<&'change str>,
    additional_group_change: Vec<UserGroupDiff<'change>>,
    new_home: Option<MaybeOwned<'change, Path>>,
    new_shell: Option<&'change str>,
}

impl UserDiff<'_> {
    fn is_empty(&self) -> bool {
        self.new_primary_group.is_none()
            && self.additional_group_change.is_empty()
            && self.new_home.is_none()
            && self.new_shell.is_none()
    }

    fn has_changes(&self) -> bool {
        !self.is_empty()
    }
}

#[derive(Debug)]
pub(crate) enum UserChange<'change> {
    Create,
    Change {
        existing: user::User,
        diff: UserDiff<'change>,
    },
}

#[derive(Debug)]
pub enum PrimaryGroupConfig<'a> {
    Automatic,
    Explicit(&'a str),
}

#[derive(Debug)]
pub struct User<'change> {
    pub name: &'change str,
    pub home: MaybeOwned<'change, Path>,
    pub primary_group: PrimaryGroupConfig<'change>,
    pub additional_groups: &'change [&'change str],
    pub shell: &'change str,
    pub system: bool,
}

impl<'change> User<'change> {
    fn diff<'u>(&self, with: &'u user::User) -> UserDiff<'change> {
        UserDiff {
            new_primary_group: match self.primary_group {
                PrimaryGroupConfig::Automatic => None,
                PrimaryGroupConfig::Explicit(primary_group) => {
                    (primary_group != with.primary_group.name).then_some(primary_group)
                }
            },
            additional_group_change: {
                let mut changes = vec![];

                for new in self.additional_groups {
                    if !with
                        .additional_groups
                        .iter()
                        .any(|group| group.name.as_str() == *new)
                    {
                        changes.push(UserGroupDiff::Add(new));
                    }
                }

                for existing in &with.additional_groups {
                    if !self.additional_groups.contains(&existing.name.as_str()) {
                        changes.push(UserGroupDiff::Remove(existing.clone()));
                    }
                }

                changes
            },
            new_home: (self.home.as_ref() != with.home).then_some(self.home.clone()),
            new_shell: (self.shell != with.shell).then_some(self.shell),
        }
    }
}

impl<'change> Plannable<'change> for User<'change> {
    fn plan<'ctx>(self: Box<Self>, ctx: &'ctx Context<'ctx>) -> Result<ChangeStatus<'change>>
    where
        'ctx: 'change,
    {
        let user = user::find_user(self.name, &ctx.elevate)?;

        if let Some(user) = user {
            let diff = self.diff(&user);
            if diff.has_changes() {
                Ok(ChangeStatus::NeedsChange(Box::new(UserPlan {
                    ctx,
                    args: *self,
                    change: UserChange::Change {
                        diff,
                        existing: user,
                    },
                })))
            } else {
                Ok(ChangeStatus::Clean)
            }
        } else {
            Ok(ChangeStatus::NeedsChange(Box::new(UserPlan {
                ctx,
                args: *self,
                change: UserChange::Create,
            })))
        }
    }
}

#[derive(Debug)]
pub(crate) struct UserPlan<'ctx, 'args> {
    pub(crate) ctx: &'ctx Context<'ctx>,
    // #[expect(unused, reason = "kept to have the same shape as other plans")]
    pub(crate) args: User<'args>,
    pub(crate) change: UserChange<'args>,
}

impl Applyable for UserPlan<'_, '_> {
    fn apply(&self) -> Result<()> {
        match self.change {
            UserChange::Create => {
                let mut args = vec![];

                let comma_group_list = self.args.additional_groups.join(",");

                args.extend(["--home-dir", self.args.home.as_ref().as_str()]);
                match self.args.primary_group {
                    PrimaryGroupConfig::Automatic => args.push("--user-group"),
                    PrimaryGroupConfig::Explicit(group) => {
                        args.extend(["--no-user-group", "--gid", group]);
                    }
                }
                args.extend(["--groups", comma_group_list.as_str()]);
                args.extend(["--shell", self.args.shell]);
                args.extend(["--create-home"]);
                args.extend(["--skel", "/dev/null"]);
                if self.args.system {
                    args.push("--system");
                }
                args.extend(["--", self.args.name]);

                cmd::Process::run("useradd", &args, &self.ctx.elevate, None)?.check()?;
            }
            UserChange::Change {
                existing: _,
                ref diff,
            } => {
                if let Some(new_primary_group) = diff.new_primary_group {
                    cmd::Process::run(
                        "usermod",
                        &["--gid", new_primary_group, "--", self.args.name],
                        &self.ctx.elevate,
                        None,
                    )?
                    .check()?;
                }

                for change in &diff.additional_group_change {
                    match *change {
                        UserGroupDiff::Add(group) => {
                            cmd::Process::run(
                                "usermod",
                                &["--append", "--groups", group, "--", self.args.name],
                                &self.ctx.elevate,
                                None,
                            )?
                            .check()?;
                        }
                        UserGroupDiff::Remove(ref group) => {
                            cmd::Process::run(
                                "usermod",
                                &["--remove", "--groups", &group.name, "--", self.args.name],
                                &self.ctx.elevate,
                                None,
                            )?
                            .check()?;
                        }
                    }
                }

                if let Some(ref new_home) = diff.new_home {
                    cmd::Process::run(
                        "usermod",
                        &[
                            "--move-home",
                            "--home",
                            new_home.as_ref().as_str(),
                            "--",
                            self.args.name,
                        ],
                        &self.ctx.elevate,
                        None,
                    )?
                    .check()?;
                }

                if let Some(new_shell) = diff.new_shell {
                    cmd::Process::run(
                        "usermod",
                        &["--shell", new_shell, "--", self.args.name],
                        &self.ctx.elevate,
                        None,
                    )?
                    .check()?;
                }
            }
        }

        Ok(())
    }
}

impl Reporter for UserPlan<'_, '_> {
    fn report(&self) -> Result<Vec<String>> {
        match self.change {
            UserChange::Create => Ok(vec!["create user".to_owned()]),
            UserChange::Change {
                ref existing,
                ref diff,
            } => {
                let mut out = vec![];

                if let Some(new_primary_group) = diff.new_primary_group {
                    out.push(format!(
                        "primary group: {} -> {}",
                        existing.primary_group, new_primary_group,
                    ));
                }

                for change in &diff.additional_group_change {
                    match *change {
                        UserGroupDiff::Add(group) => {
                            out.push(format!("add additional group {group}"));
                        }
                        UserGroupDiff::Remove(ref group) => {
                            out.push(format!("remove additional group {group}"));
                        }
                    }
                }

                if let Some(ref new_home) = diff.new_home {
                    out.push(format!("home: {} -> {}", existing.home, new_home.as_ref()));
                }

                if let Some(new_shell) = diff.new_shell {
                    out.push(format!("shell: {} -> {}", existing.shell, new_shell));
                }

                Ok(out)
            }
        }
    }
}

#[derive(Debug)]
pub(crate) enum GroupChange {
    Create,
}

#[derive(Debug)]
pub struct Group<'change> {
    pub name: &'change str,
    pub system: bool,
}

impl<'change> Plannable<'change> for Group<'change> {
    fn plan<'ctx>(self: Box<Self>, ctx: &'ctx Context<'ctx>) -> Result<ChangeStatus<'change>>
    where
        'ctx: 'change,
    {
        let group = user::find_group(self.name, &ctx.elevate)?;

        if let Some(_group) = group {
            Ok(ChangeStatus::Clean)
        } else {
            Ok(ChangeStatus::NeedsChange(Box::new(GroupPlan {
                ctx,
                args: *self,
                change: GroupChange::Create,
            })))
        }
    }
}

#[derive(Debug)]
pub(crate) struct GroupPlan<'ctx, 'args> {
    pub(crate) ctx: &'ctx Context<'ctx>,
    // #[expect(unused, reason = "kept to have the same shape as other plans")]
    pub(crate) args: Group<'args>,
    pub(crate) change: GroupChange,
}

impl Applyable for GroupPlan<'_, '_> {
    fn apply(&self) -> Result<()> {
        match self.change {
            GroupChange::Create => {
                let mut args = vec![];

                if self.args.system {
                    args.push("--system");
                }
                args.extend(["--", self.args.name]);

                cmd::Process::run("groupadd", &args, &self.ctx.elevate, None)?.check()?;
            }
        }

        Ok(())
    }
}

impl Reporter for GroupPlan<'_, '_> {
    fn report(&self) -> Result<Vec<String>> {
        match self.change {
            GroupChange::Create => Ok(vec!["create group".to_owned()]),
        }
    }
}
