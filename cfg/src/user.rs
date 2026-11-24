#![expect(
    clippy::large_include_file,
    reason = "Scripts and executables are part of the binary on purpose"
)]

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use dedent::dedent;
use include_dir::DirEntry;
use include_dir::include_dir;
use serde::Serialize;

use maybeowned::prelude::*;
use oxidizer::prelude::*;

use crate::{
    hosts::{MachineConfig, UserConfig, UserContext},
    secret::Secrets,
};

static BINS: include_dir::Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../files/bin");

static SCRIPTS: include_dir::Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../files/scripts");

type PackageList<'pkgs> =
    HashMap<MaybeOwned<'pkgs, str>, MaybeOwned<'pkgs, [MaybeOwned<'pkgs, str>]>>;

const AUTHORIZED_KEYS: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../assets/ssh_pubkey"));

pub(crate) struct UserDirectories {
    pub(crate) config: PathBuf,
    pub(crate) bin: PathBuf,
    pub(crate) scripts: PathBuf,
    pub(crate) tmp: PathBuf,
    pub(crate) download: PathBuf,
    pub(crate) systemd: PathBuf,
    pub(crate) systemd_user_units: PathBuf,
    pub(crate) local: PathBuf,
    pub(crate) data: PathBuf,
    pub(crate) state: PathBuf,
    pub(crate) applications: PathBuf,
}

pub struct UserResult<'pkgs> {
    pub packages: HashMap<MaybeOwned<'pkgs, str>, MaybeOwned<'pkgs, [MaybeOwned<'pkgs, str>]>>,
}

pub async fn configure<'elevate: 'action, 'action, 'user: 'action, 'machine, 'pkgs>(
    plan: &mut Block<'_, 'elevate, 'pkgs, 'action, '_, Secrets>,
    root: &'elevate Elevate,
    user: &'elevate UserContext,
    machine: &'machine MachineConfig,
) -> Result<UserResult<'pkgs>> {
    let mut packages = HashMap::new();
    let mut services = vec![];

    let UserContext {
        simplest_elevate: ref user_elevate,
        ref strict_elevate,
        config: ref user,
    } = *user;

    let home = PathBuf::from("/home/").join(&user.name);

    plan.add(
        "install zsh",
        root,
        Packages {
            install: Ref(&[Ref("zsh")]),
            reason: InstallReason::Explicit,
        },
    );

    plan.add(
        "create user",
        root,
        User {
            name: &user.name,
            home: Owned(home.clone()),
            primary_group: PrimaryGroupConfig::Automatic,
            additional_groups: &[
                "libvirt",
                "wheel",
                "wireshark",
                "sudonopw",
                "games",
                "kvm",
                "video",
                "ssh",
                "docker",
            ],
            shell: "/usr/bin/zsh",
            system: false,
        },
    );

    if user.autologin {
        autologin(plan, root, user);
    }

    let user_directories = {
        let config = home.join(".config");
        let systemd = config.join("systemd");
        let local = home.join(".local");
        let data = local.join("share");

        UserDirectories {
            config,
            bin: home.join("bin"),
            scripts: home.join("scripts"),
            tmp: home.join("tmp"),
            download: home.join("download"),
            systemd_user_units: systemd.join("user"),
            state: local.join("state"),
            applications: data.join("applications"),
            data,
            systemd,
            local,
        }
    };

    directories(plan, user_elevate, user, &user_directories);

    let (required_packages, additional_services) = dotfiles(
        plan,
        user_elevate,
        strict_elevate,
        user,
        machine,
        &home,
        &user_directories,
    )?;

    services.extend(additional_services);
    packages.extend(required_packages);

    let dependencies = Box::pin(executables(plan, user_elevate, user, &user_directories)).await?;

    packages.extend(dependencies);

    packages.insert(
        Ref("pass"),
        Ref([Ref("pass"), Ref("passff-host")].as_slice()),
    );

    packages.insert(
        Ref("theming"),
        Ref([Ref("breeze"), Ref("breeze-grub"), Ref("breeze-gtk")].as_slice()),
    );

    wallpaper(plan, user_elevate, user, &user_directories);
    user_services(plan, user_elevate, user, &user_directories, &services);

    let ssh_directory = home.join(".ssh");
    plan.add(
        "create ~/.ssh directory",
        user_elevate,
        Directory {
            path: Owned(ssh_directory.clone()),
            on_symlink: SymlinkHandling::Proceed,
            on_file: FileHandling::Proceed,
            permissions: FilePermissions {
                owner: Uid::from(user.name.clone()),
                group: Gid::from(user.name.clone()),
                mode: FileMode(0o700),
            },
        },
    );

    plan.add(
        "manage user authorized keys",
        user_elevate,
        File {
            path: Owned(ssh_directory.join("authorized_keys")),
            permissions: FilePermissions {
                owner: Uid::from(user.name.clone()),
                group: Gid::from(user.name.clone()),
                mode: FileMode(0o600),
            },
            on_directory: DirectoryHandling::Bail,
            on_symlink: SymlinkHandling::Proceed,
            binary: false,
            secret: false,
            ignore_checksum_changes: false,
            content: FileContent::Bytes(Ref(AUTHORIZED_KEYS)),
        },
    );

    Ok(UserResult { packages })
}

pub(crate) fn directories<'elevate: 'action, 'action, 'user, 'machine>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    user_elevate: &'elevate Elevate,
    user: &'elevate UserConfig,
    user_directories: &UserDirectories,
) {
    plan.block("user directories", |plan| {
        plan.add(
            "create user config directory",
            user_elevate,
            Directory {
                path: Owned(user_directories.config.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("create download directory"),
            user_elevate,
            Directory {
                path: Owned(user_directories.download.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("create tmp directory"),
            user_elevate,
            Directory {
                path: Owned(user_directories.tmp.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("create scripts directory"),
            user_elevate,
            Directory {
                path: Owned(user_directories.scripts.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("create bin directory"),
            user_elevate,
            Directory {
                path: Owned(user_directories.bin.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("create systemd user directory"),
            user_elevate,
            Directory {
                path: Owned(user_directories.systemd.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("create systemd user unit directory"),
            user_elevate,
            Directory {
                path: Owned(user_directories.systemd_user_units.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("create local directory"),
            user_elevate,
            Directory {
                path: Owned(user_directories.local.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("create data directory"),
            user_elevate,
            Directory {
                path: Owned(user_directories.data.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("create state directory"),
            user_elevate,
            Directory {
                path: Owned(user_directories.state.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("create applications directory"),
            user_elevate,
            Directory {
                path: Owned(user_directories.applications.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );
    });
}

pub(crate) fn autologin<'elevate: 'action, 'action, 'user, 'machine>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    root: &'elevate Elevate,
    user: &'user UserConfig,
) {
    plan.block("autologin", |plan| {
        let getty_base = PathBuf::from(format!(
            "/etc/systemd/system/getty@tty{vt}.service.d/",
            vt = user.vt
        ));

        plan.add(
            "create directory for getty autologin",
            root,
            Directory {
                path: Owned(getty_base.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::root(),
                    group: Gid::root(),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            "enable getty autologin",
            root,
            File {
                path: Owned(getty_base.join("override.conf")),
                permissions: FilePermissions {
                    owner: Uid::root(),
                    group: Gid::root(),
                    mode: FileMode(0o644),
                },
                on_directory: DirectoryHandling::Bail,
                on_symlink: SymlinkHandling::Proceed,
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Lines(Owned(vec![
                    Ref("[Service]"),
                    Ref("ExecStart="),
                    Owned(format!(
                        "ExecStart=-/sbin/agetty --autologin {name} --noclear %I $TERM",
                        name = user.name
                    )),
                ])),
            },
        );
    });
}

pub(crate) fn dotfiles<'elevate: 'action, 'action, 'user, 'machine, 'pkgs>(
    plan: &mut Block<'_, 'elevate, 'pkgs, 'action, '_, Secrets>,
    user_elevate: &'elevate Elevate,
    strict_elevate: &'elevate Elevate,
    user: &'user UserConfig,
    machine: &MachineConfig,
    home: &Path,
    directories: &UserDirectories,
) -> Result<(PackageList<'pkgs>, Vec<MaybeOwned<'static, str>>)> {
    #[expect(
        clippy::struct_excessive_bools,
        reason = "i don't want logic in the templates"
    )]
    #[derive(Serialize)]
    struct TemplateContext<'a> {
        gpg_agent_enabled: bool,
        ssh_agent_enabled: bool,
        gpg_agent_for_ssh: bool,
        vt: usize,
        fullname: &'a str,
        mail: &'a str,
        keyboard_repeat_delay: usize,
        keyboard_repeat_rate: usize,
        font_size: usize,
        workspace_sway_config: String,
        is_laptop: bool,
        is_workstation: bool,
        is_tv: bool,
    }

    let mut packages = HashMap::new();
    let mut services = Vec::new();

    let template_context = TemplateContext {
        font_size: machine.font_size,
        workspace_sway_config: user.workspace.to_sway_config(),
        is_laptop: machine.machine_type.is_laptop(),
        is_workstation: machine.machine_type.is_workstation(),
        is_tv: machine.machine_type.is_tv(),
        gpg_agent_enabled: user.gpg.as_ref().is_some_and(|config| config.agent_enabled),
        gpg_agent_for_ssh: user
            .gpg
            .as_ref()
            .is_some_and(|config| config.gpg_agent_for_ssh),
        ssh_agent_enabled: user.ssh_agent_enabled,
        vt: user.vt,
        fullname: user.fullname.as_str(),
        mail: user.mail.as_str(),
        keyboard_repeat_delay: user.keyboard.repeat_delay,
        keyboard_repeat_rate: user.keyboard.repeat_rate,
    };

    let template_context = tera::Context::from_serialize(&template_context)?;

    plan.block("rofi", |plan| {
        let config_dir = directories.config.join("rofi");

        packages.insert(Ref("rofi"), Ref([Ref("rofi")].as_slice()));

        plan.add(
            Ref("create config directory for rofi"),
            user_elevate,
            Directory {
                path: Owned(config_dir.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("add config for rofi"),
            user_elevate,
            File {
                path: Owned(config_dir.join("config")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/rofi/config"
                )))),
            },
        );
    });

    plan.try_block("i3status-rust", |plan| {
        let config_dir = directories.config.join("i3status-rust");

        packages.insert(Ref("i3status-rust"), Ref([Ref("i3status-rust")].as_slice()));

        plan.add(
            Ref("create config directory for i3status-rust"),
            user_elevate,
            Directory {
                path: Owned(config_dir.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("add config for i3status-rust"),
            user_elevate,
            File {
                path: Owned(config_dir.join("config.toml")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Owned(
                    tera::Tera::one_off(
                        include_str!(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/../files/i3status-rust/config.toml.j2"
                        )),
                        &template_context,
                        false,
                    )?
                    .into_bytes(),
                )),
            },
        );

        let icons_dir = config_dir.join("icons");

        plan.add(
            Ref("create icons subdirectory for i3status-rust"),
            user_elevate,
            Directory {
                path: Owned(icons_dir.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("add awesome icons for i3status-rust"),
            user_elevate,
            File {
                path: Owned(icons_dir.join("awesome.toml")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/i3status-rust/icons/awesome.toml"
                )))),
            },
        );

        Ok(())
    })?;

    plan.block("helix", |plan| {
        let config_dir = directories.config.join("helix");

        packages.insert(Ref("helix"), Ref([Ref("helix")].as_slice()));

        plan.add(
            Ref("create config directory for helix"),
            user_elevate,
            Directory {
                path: Owned(config_dir.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("add config for helix"),
            user_elevate,
            File {
                path: Owned(config_dir.join("config.toml")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/helix/config.toml"
                )))),
            },
        );

        plan.add(
            Ref("add language configs for helix"),
            user_elevate,
            File {
                path: Owned(config_dir.join("languages.toml")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/helix/languages.toml"
                )))),
            },
        );
    });

    plan.try_block("git", |plan| {
        let config_dir = directories.config.join("git");

        packages.insert(
            Ref("git"),
            Ref([
                Ref("git"),
                Ref("qgit"),
                Ref("git-delta"),
                Ref("git-filter-repo"),
            ]
            .as_slice()),
        );

        plan.add(
            Ref("create config directory for git"),
            user_elevate,
            Directory {
                path: Owned(config_dir.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("add config for git"),
            user_elevate,
            File {
                path: Owned(config_dir.join("config")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Owned(
                    tera::Tera::one_off(
                        include_str!(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/../files/git/gitconfig.j2"
                        )),
                        &template_context,
                        false,
                    )?
                    .into_bytes(),
                )),
            },
        );

        plan.add(
            Ref("add global ignore for git"),
            user_elevate,
            File {
                path: Owned(config_dir.join("ignore")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/git/ignore"
                ))
                .as_slice())),
            },
        );

        Ok(())
    })?;

    plan.block("tmux", |plan| {
        let config_dir = directories.config.join("tmux");

        packages.insert(Ref("tmux"), Ref([Ref("tmux")].as_slice()));

        plan.add(
            Ref("create config directory for tmux"),
            user_elevate,
            Directory {
                path: Owned(config_dir.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("add config for tmux"),
            user_elevate,
            File {
                path: Owned(config_dir.join("tmux.conf")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/tmux/tmux.conf"
                )))),
            },
        );
    });

    plan.try_block("sway", |plan| {
        let config_dir = directories.config.join("sway");

        packages.insert(
            Ref("sway"),
            Ref([
                Ref("sway"),
                Ref("swaybg"),
                Ref("swaylock"),
                Ref("swayidle"),
                Ref("xorg-xwayland"),
                Ref("xorg-xlsclients"),
            ]
            .as_slice()),
        );

        plan.add(
            Ref("create config directory for sway"),
            user_elevate,
            Directory {
                path: Owned(config_dir.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("add config for sway"),
            user_elevate,
            File {
                path: Owned(config_dir.join("config")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Owned(
                    tera::Tera::one_off(
                        include_str!(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/../files/sway/config.j2"
                        )),
                        &template_context,
                        false,
                    )?
                    .into_bytes(),
                )),
            },
        );

        let scripts_dir = config_dir.join("scripts");
        plan.add(
            Ref("create scripts directory for sway"),
            user_elevate,
            Directory {
                path: Owned(scripts_dir.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("add workspace switcher script for sway"),
            user_elevate,
            File {
                path: Owned(scripts_dir.join("swap-from-workspace.py")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/sway/scripts/swap-from-workspace.py"
                )))),
            },
        );

        plan.add(
            Ref("add sway service"),
            user_elevate,
            File {
                path: Owned(directories.systemd_user_units.join("sway.service")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/services/sway.service"
                )))),
            },
        );

        Ok(())
    })?;

    plan.try_block("zsh", |plan| {
        let config_dir = directories.config.join("zsh");

        packages.insert(
            Ref("zsh"),
            Ref([
                Ref("zsh"),
                Ref("zsh-syntax-highlighting"),
                Ref("zsh-completions"),
                Ref("eza"),                // alias for ls
                Ref("python-ruamel-yaml"), // json2yaml alias
                Ref("fzf"),                // fzf integration
            ]
            .as_slice()),
        );

        plan.add(
            Ref("add zshenv"),
            user_elevate,
            File {
                path: Owned(home.join(".zshenv")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/zsh/zshenv"
                )))),
            },
        );

        plan.add(
            Ref("create config directory for zsh"),
            user_elevate,
            Directory {
                path: Owned(config_dir.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("add zprofile"),
            user_elevate,
            File {
                path: Owned(config_dir.join(".zprofile")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Owned(
                    tera::Tera::one_off(
                        include_str!(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/../files/zsh/zprofile.j2"
                        )),
                        &template_context,
                        false,
                    )?
                    .into_bytes(),
                )),
            },
        );

        plan.add(
            Ref("add zshrc"),
            user_elevate,
            File {
                path: Owned(config_dir.join(".zshrc")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Owned(
                    tera::Tera::one_off(
                        include_str!(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/../files/zsh/zshrc.j2"
                        )),
                        &template_context,
                        false,
                    )?
                    .into_bytes(),
                )),
            },
        );

        Ok(())
    })?;

    plan.try_block("alacritty", |plan| {
        let config_dir = directories.config.join("alacritty");

        packages.insert(Ref("alacritty"), Ref([Ref("alacritty")].as_slice()));

        plan.add(
            Ref("create config directory for alacritty"),
            user_elevate,
            Directory {
                path: Owned(config_dir.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("add config for alacritty"),
            user_elevate,
            File {
                path: Owned(config_dir.join("config.toml")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Owned(
                    tera::Tera::one_off(
                        include_str!(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/../files/alacritty/alacritty.toml.j2"
                        )),
                        &template_context,
                        false,
                    )?
                    .into_bytes(),
                )),
            },
        );

        let themes_dir = config_dir.join("themes");
        plan.add(
            Ref("create themes directory for alacritty"),
            user_elevate,
            Directory {
                path: Owned(themes_dir.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("add monokai theme for alacritty"),
            user_elevate,
            File {
                path: Owned(themes_dir.join("monokai.toml")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/alacritty/themes/monokai.toml"
                )))),
            },
        );

        Ok(())
    })?;

    plan.try_block("gpg", |plan| {
        let config_dir = directories.state.join("gnupg");

        packages.insert(
            Ref("gpg"),
            Ref([
                Ref("gnupg"),
                Ref("paperkey"),
                Ref("yubikey-manager"),
                Ref("yubikey-touch-detector"),
                Ref("pinentry"),
            ]
            .as_slice()),
        );

        plan.add(
            Ref("create config directory for gpg"),
            user_elevate,
            Directory {
                path: Owned(config_dir.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o700),
                },
            },
        );

        plan.add(
            Ref("add config for gpg"),
            user_elevate,
            File {
                path: Owned(config_dir.join("gpg.conf")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Owned(
                    tera::Tera::one_off(
                        include_str!(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/../files/gnupg/gpg.conf.j2"
                        )),
                        &template_context,
                        false,
                    )?
                    .into_bytes(),
                )),
            },
        );

        plan.add(
            Ref("add agent config for gpg"),
            user_elevate,
            File {
                path: Owned(config_dir.join("gpg-agent.conf")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Owned(
                    tera::Tera::one_off(
                        include_str!(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/../files/gnupg/gpg-agent.conf.j2"
                        )),
                        &template_context,
                        false,
                    )?
                    .into_bytes(),
                )),
            },
        );

        plan.add(
            Ref("add dirmngr config for gpg"),
            user_elevate,
            File {
                path: Owned(config_dir.join("dirmngr.conf")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/gnupg/dirmngr.conf"
                )))),
            },
        );

        plan.add(
            Ref("add scdaemon config for gpg"),
            user_elevate,
            File {
                path: Owned(config_dir.join("scdaemon.conf")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/gnupg/scdaemon.conf"
                )))),
            },
        );

        let gpg_agent_service_name = "gpg-agent.service";
        let gpg_agent_service_path = directories.systemd_user_units.join(gpg_agent_service_name);

        let yubikey_service_name = "yubikey-touch-detector.service";
        let yubikey_service_path = directories.systemd_user_units.join(yubikey_service_name);

        if user.gpg.as_ref().is_some_and(|gpg| gpg.agent_enabled) {
            plan.add(
                Ref("add gpg-agent service"),
                user_elevate,
                File {
                    path: Owned(gpg_agent_service_path),
                    on_symlink: SymlinkHandling::Proceed,
                    on_directory: DirectoryHandling::Bail,
                    permissions: FilePermissions {
                        owner: Uid::from(user.name.clone()),
                        group: Gid::from(user.name.clone()),
                        mode: FileMode(0o644),
                    },
                    binary: false,
                    secret: false,
                    ignore_checksum_changes: false,
                    content: FileContent::Bytes(Ref(include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../files/services/gpg-agent.service"
                    )))),
                },
            );

            services.push(Ref(gpg_agent_service_name));

            plan.add(
                Ref("add yubikey-touch-detector service"),
                user_elevate,
                File {
                    path: Owned(yubikey_service_path),
                    on_symlink: SymlinkHandling::Proceed,
                    on_directory: DirectoryHandling::Bail,
                    permissions: FilePermissions {
                        owner: Uid::from(user.name.clone()),
                        group: Gid::from(user.name.clone()),
                        mode: FileMode(0o644),
                    },
                    binary: false,
                    secret: false,
                    ignore_checksum_changes: false,
                    content: FileContent::Bytes(Ref(include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../files/services/yubikey-touch-detector.service"
                    )))),
                },
            );

            services.push(Ref(yubikey_service_name));
        } else {
            plan.add(
                Ref("user service gpg-agent"),
                user_elevate,
                Remove {
                    path: Owned(gpg_agent_service_path),
                    on_directory: DirectoryHandling::Bail,
                    on_file: FileHandling::Proceed,
                    on_symlink: SymlinkHandling::Proceed,
                },
            );

            plan.add(
                Ref("user service yubikey-touch-detector"),
                user_elevate,
                Remove {
                    path: Owned(yubikey_service_path),
                    on_directory: DirectoryHandling::Bail,
                    on_file: FileHandling::Proceed,
                    on_symlink: SymlinkHandling::Proceed,
                },
            );
        }

        plan.add(
            "import my gpg key",
            user_elevate,
            GpgKey {
                content: include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/gpgkeys/hannes@hkoerber.de.gpg.asc"
                )),
                gnupg_directory: Owned(home.join(".local/state/gnupg/")),
                // run as the actual user, not using root
                elevate_apply: strict_elevate,
                fingerprint: GpgFingerprint("973AE48D71B76735C4712B5BB5C002530C6A2053".to_owned()),
                trust: GpgTrust::Ultimately,
            },
        );

        Ok(())
    })?;

    plan.block("rust", |plan| {
        let config_dir = directories.state.join("cargo");

        packages.insert(Ref("rust"), Ref([Ref("rustup"), Ref("mold")].as_slice()));

        plan.add(
            Ref("create config directory for cargo"),
            user_elevate,
            Directory {
                path: Owned(config_dir.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("add config for cargo"),
            user_elevate,
            File {
                path: Owned(config_dir.join("config.toml")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/cargo/config.toml"
                )))),
            },
        );
    });

    plan.block("gnome-keyring", |plan| {
        packages.insert(
            Ref("gnome-keyring"),
            Ref([Ref("gnome-keyring"), Ref("seahorse")].as_slice()),
        );

        let service_name = "gnome-keyring.service";

        plan.add(
            Ref("add gnome-keyring service"),
            user_elevate,
            File {
                path: Owned(directories.systemd_user_units.join(service_name)),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/services/gnome-keyring.service"
                )))),
            },
        );

        services.push(Ref(service_name));
    });

    plan.block("kdeconnect", |plan| {
        packages.insert(Ref("kdeconnect"), Ref([Ref("kdeconnect")].as_slice()));

        let service_name = "kdeconnect.service";

        plan.add(
            Ref("add kdeconnect service"),
            user_elevate,
            File {
                path: Owned(directories.systemd_user_units.join(service_name)),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/services/kdeconnect.service"
                )))),
            },
        );

        services.push(Ref(service_name));
    });

    plan.block("keepassxc", |plan| {
        let service_name = "keepassxc.service";
        let service_path = directories.systemd_user_units.join(service_name);

        if user.keepassxc.enabled() {
            packages.insert(Ref("keepassxc"), Ref([Ref("keepassxc")].as_slice()));

            plan.add(
                Ref("add keepassxc service"),
                user_elevate,
                File {
                    path: Owned(service_path),
                    on_symlink: SymlinkHandling::Proceed,
                    on_directory: DirectoryHandling::Bail,
                    permissions: FilePermissions {
                        owner: Uid::from(user.name.clone()),
                        group: Gid::from(user.name.clone()),
                        mode: FileMode(0o644),
                    },
                    binary: false,
                    secret: false,
                    ignore_checksum_changes: false,
                    content: FileContent::Bytes(Ref(include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../files/services/keepassxc.service"
                    )))),
                },
            );

            services.push(Ref(service_name));
        } else {
            plan.add(
                Ref("user service keepassxc"),
                user_elevate,
                Remove {
                    path: Owned(service_path),
                    on_directory: DirectoryHandling::Bail,
                    on_file: FileHandling::Proceed,
                    on_symlink: SymlinkHandling::Proceed,
                },
            );
        }
    });

    plan.block("nextcloud", |plan| {
        let service_name = "nextcloud.service";
        let service_path = directories.systemd_user_units.join(service_name);

        if user.nextcloud.enabled() {
            packages.insert(Ref("nextloud"), Ref([Ref("nextcloud-client")].as_slice()));

            plan.add(
                Ref("add nextcloud service"),
                user_elevate,
                File {
                    path: Owned(service_path),
                    on_symlink: SymlinkHandling::Proceed,
                    on_directory: DirectoryHandling::Bail,
                    permissions: FilePermissions {
                        owner: Uid::from(user.name.clone()),
                        group: Gid::from(user.name.clone()),
                        mode: FileMode(0o644),
                    },
                    binary: false,
                    secret: false,
                    ignore_checksum_changes: false,
                    content: FileContent::Bytes(Ref(include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../files/services/nextcloud.service"
                    )))),
                },
            );

            services.push(Ref(service_name));
        } else {
            plan.add(
                Ref("user service nextcloud"),
                user_elevate,
                Remove {
                    path: Owned(service_path),
                    on_directory: DirectoryHandling::Bail,
                    on_file: FileHandling::Proceed,
                    on_symlink: SymlinkHandling::Proceed,
                },
            );
        }
    });

    plan.block("screencfg", |plan| {
        let service_name = "screencfg.service";

        plan.add(
            Ref("add screencfg service"),
            user_elevate,
            File {
                path: Owned(directories.systemd_user_units.join(service_name)),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/services/screencfg.service"
                )))),
            },
        );

        services.push(Ref(service_name));
    });

    plan.block("spotify", |plan| {
        let service_name = "spotify.service";

        plan.add(
            Ref("add spotify service"),
            user_elevate,
            File {
                path: Owned(directories.systemd_user_units.join(service_name)),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/services/spotify.service"
                )))),
            },
        );

        services.push(Ref(service_name));
    });

    plan.block("discord", |plan| {
        let service_name = "discord.service";
        let service_path = directories.systemd_user_units.join(service_name);

        if machine.roles.has_gaming() {
            packages.insert(Ref("discord"), Ref(&[Ref("discord")]));

            plan.add(
                Ref("add discord service"),
                user_elevate,
                File {
                    path: Owned(service_path),
                    on_symlink: SymlinkHandling::Proceed,
                    on_directory: DirectoryHandling::Bail,
                    permissions: FilePermissions {
                        owner: Uid::from(user.name.clone()),
                        group: Gid::from(user.name.clone()),
                        mode: FileMode(0o644),
                    },
                    binary: false,
                    secret: false,
                    ignore_checksum_changes: false,
                    content: FileContent::Bytes(Ref(include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../files/services/discord.service"
                    )))),
                },
            );

            services.push(Ref(service_name));
        } else {
            plan.add(
                Ref("user service discord"),
                user_elevate,
                Remove {
                    path: Owned(service_path),
                    on_directory: DirectoryHandling::Bail,
                    on_file: FileHandling::Proceed,
                    on_symlink: SymlinkHandling::Proceed,
                },
            );
        }
    });

    plan.block("steam", |plan| {
        let service_name = "steam.service";
        let service_path = directories.systemd_user_units.join(service_name);

        if user.steam.enabled() {
            packages.insert(
                Ref("steam"),
                Ref([
                    Ref("steam"),
                    Ref("lib32-pango"),
                    Ref("pango"),
                    Ref("ttf-liberation"),
                ]
                .as_slice()),
            );

            plan.add(
                Ref("add steam service"),
                user_elevate,
                File {
                    path: Owned(service_path),
                    on_symlink: SymlinkHandling::Proceed,
                    on_directory: DirectoryHandling::Bail,
                    permissions: FilePermissions {
                        owner: Uid::from(user.name.clone()),
                        group: Gid::from(user.name.clone()),
                        mode: FileMode(0o644),
                    },
                    binary: false,
                    secret: false,
                    ignore_checksum_changes: false,
                    content: FileContent::Bytes(Ref(include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../files/services/steam.service"
                    )))),
                },
            );

            services.push(Ref(service_name));
        } else {
            plan.add(
                Ref("user service steam"),
                user_elevate,
                Remove {
                    path: Owned(service_path),
                    on_directory: DirectoryHandling::Bail,
                    on_file: FileHandling::Proceed,
                    on_symlink: SymlinkHandling::Proceed,
                },
            );
        }
    });

    plan.block("laptop-lid", |plan| {
        let service_name = "laptop-lid.service";
        let service_path = directories.systemd_user_units.join(service_name);

        if machine.machine_type.is_laptop() {
            plan.add(
                Ref("add laptop-lid service"),
                user_elevate,
                File {
                    path: Owned(service_path),
                    on_symlink: SymlinkHandling::Proceed,
                    on_directory: DirectoryHandling::Bail,
                    permissions: FilePermissions {
                        owner: Uid::from(user.name.clone()),
                        group: Gid::from(user.name.clone()),
                        mode: FileMode(0o644),
                    },
                    binary: false,
                    secret: false,
                    ignore_checksum_changes: false,
                    content: FileContent::Bytes(Ref(include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../files/services/laptop-lid.service"
                    )))),
                },
            );

            services.push(Ref(service_name));
        } else {
            plan.add(
                Ref("user service laptop-lid"),
                user_elevate,
                Remove {
                    path: Owned(service_path),
                    on_directory: DirectoryHandling::Bail,
                    on_file: FileHandling::Proceed,
                    on_symlink: SymlinkHandling::Proceed,
                },
            );
        }
    });

    plan.block("gammastep", |plan| {
        let service_name = "gammastep.service";
        let service_path = directories.systemd_user_units.join(service_name);
        let config_path = directories.config.join("gammastep.conf");

        if !machine.machine_type.is_tv() {
            packages.insert(Ref("gammastep"), Ref([Ref("gammastep")].as_slice()));

            plan.add(
                Ref("add gammastep config"),
                user_elevate,
                File {
                    path: Owned(config_path),
                    on_symlink: SymlinkHandling::Proceed,
                    on_directory: DirectoryHandling::Bail,
                    permissions: FilePermissions {
                        owner: Uid::from(user.name.clone()),
                        group: Gid::from(user.name.clone()),
                        mode: FileMode(0o644),
                    },
                    binary: false,
                    secret: false,
                    ignore_checksum_changes: false,
                    content: FileContent::Bytes(Ref(include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../files/gammastep/gammastep.conf"
                    )))),
                },
            );

            plan.add(
                Ref("add gammastep service"),
                user_elevate,
                File {
                    path: Owned(service_path),
                    on_symlink: SymlinkHandling::Proceed,
                    on_directory: DirectoryHandling::Bail,
                    permissions: FilePermissions {
                        owner: Uid::from(user.name.clone()),
                        group: Gid::from(user.name.clone()),
                        mode: FileMode(0o644),
                    },
                    binary: false,
                    secret: false,
                    ignore_checksum_changes: false,
                    content: FileContent::Bytes(Ref(include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../files/services/gammastep.service"
                    )))),
                },
            );

            services.push(Ref(service_name));
        } else {
            plan.add(
                Ref("user service gammastep"),
                user_elevate,
                Remove {
                    path: Owned(service_path),
                    on_directory: DirectoryHandling::Bail,
                    on_file: FileHandling::Proceed,
                    on_symlink: SymlinkHandling::Proceed,
                },
            );

            plan.add(
                Ref("remove config for gammastep"),
                user_elevate,
                Remove {
                    path: Owned(config_path),
                    on_symlink: SymlinkHandling::Proceed,
                    on_directory: DirectoryHandling::Bail,
                    on_file: FileHandling::Proceed,
                },
            );
        }
    });

    plan.block("workstation-mgr", |plan| {
        let service_name = "workstation-mgr.service";

        plan.add(
            Ref("add workstation-mgr service"),
            user_elevate,
            File {
                path: Owned(directories.systemd_user_units.join(service_name)),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/services/workstation-mgr.service"
                )))),
            },
        );

        services.push(Ref(service_name));
    });

    plan.try_block("dunst", |plan| {
        packages.insert(Ref("dunst"), Ref([Ref("dunst")].as_slice()));

        plan.add(
            Ref("add dunst config"),
            user_elevate,
            File {
                path: Owned(directories.config.join("dunstrc")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Owned(
                    tera::Tera::one_off(
                        include_str!(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/../files/dunst/dunstrc.j2"
                        )),
                        &template_context,
                        false,
                    )?
                    .into_bytes(),
                )),
            },
        );

        let service_name = "dunst.service";

        plan.add(
            Ref("add dunst service"),
            user_elevate,
            File {
                path: Owned(directories.systemd_user_units.join(service_name)),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/services/dunst.service"
                )))),
            },
        );

        services.push(Ref(service_name));

        Ok(())
    })?;

    plan.block("firefox", |plan| {
        packages.insert(Ref("firefox"), Ref([Ref("firefox")].as_slice()));

        plan.add(
            Ref("add firefox desktop entry"),
            user_elevate,
            File {
                path: Owned(directories.applications.join("firefox.desktop")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/firefox/firefox.desktop"
                )))),
            },
        );

        let mozilla_dir = home.join(".mozilla");
        let firefox_dir = mozilla_dir.join("firefox");

        plan.add(
            Ref("create mozilla base directory"),
            user_elevate,
            Directory {
                path: Owned(mozilla_dir),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("create firefox base directory"),
            user_elevate,
            Directory {
                path: Owned(firefox_dir.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            },
        );

        plan.add(
            Ref("add firefox service"),
            user_elevate,
            File {
                path: Owned(directories.systemd_user_units.join("firefox@.service")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/services/firefox@.service"
                )))),
            },
        );

        for profile in &user.firefox.profiles {
            let profile_dir = firefox_dir.join(format!("profile-{}", profile.name));
            let chrome_dir = profile_dir.join("chrome");

            services.push(Owned(format!("firefox@{name}.service", name = profile.name)));

            plan.add(
                Owned(format!(
                    "create firefox profile directory for {}",
                    profile.name
                )),
                user_elevate,
                Directory {
                    path: Owned(profile_dir),
                    on_symlink: SymlinkHandling::Proceed,
                    on_file: FileHandling::Proceed,
                    permissions: FilePermissions {
                        owner: Uid::from(user.name.clone()),
                        group: Gid::from(user.name.clone()),
                        mode: FileMode(0o755),
                    },
                },
            );

            plan.add(
                Owned(format!(
                    "create firefox chrome directory for {}",
                    profile.name
                )),
                user_elevate,
                Directory {
                    path: Owned(chrome_dir.clone()),
                    on_symlink: SymlinkHandling::Proceed,
                    on_file: FileHandling::Proceed,
                    permissions: FilePermissions {
                        owner: Uid::from(user.name.clone()),
                        group: Gid::from(user.name.clone()),
                        mode: FileMode(0o755),
                    },
                },
            );

            if profile.manage_css {
                plan.add(
                    Owned(format!(
                        "create firefox userChrome.css for {}",
                        profile.name
                    )),
                    user_elevate,
                    File {
                        path: Owned(chrome_dir.join("userChrome.css")),
                        on_symlink: SymlinkHandling::Proceed,
                        on_directory: DirectoryHandling::Proceed,
                        permissions: FilePermissions {
                            owner: Uid::from(user.name.clone()),
                            group: Gid::from(user.name.clone()),
                            mode: FileMode(0o644),
                        },
                        binary: false,
                        secret: false,
                        ignore_checksum_changes: false,
                        content: FileContent::Bytes(Ref(
                            dedent!(
                                r##"
                                    /* hides the native tabs */
                                    #TabsToolbar {
                                      visibility: collapse;
                                    }

                                    /* removes the header for the sidebar that only shows "tree style tab" anyway */
                                    #sidebar-panel-header {
                                      visibility: collapse;
                                    }

                                    /* removes the weird padding between sidebar and left window border */
                                    #sidebar-box {
                                      padding-left: 0 !important;
                                    }
                                "##
                            )
                            .as_bytes(),
                        )),
                    },
                );
            }
        }
    });

    Ok((packages, services))
}

pub(crate) async fn executables<'elevate: 'action, 'action, 'user, 'machine, 'pkgs>(
    plan: &mut Block<'_, 'elevate, 'pkgs, 'action, '_, Secrets>,
    user_elevate: &'elevate Elevate,
    user: &'user UserConfig,
    user_directories: &UserDirectories,
) -> Result<HashMap<MaybeOwned<'pkgs, str>, MaybeOwned<'pkgs, [MaybeOwned<'pkgs, str>]>>> {
    plan.try_block_async_with_value("executables", async |plan| {
        let mut packages = HashMap::new();

        let scripts: Vec<(&str, &[u8])> = SCRIPTS
            .entries()
            .iter()
            .map(|entry| match *entry {
                DirEntry::Dir(ref _d) => Err(anyhow!("subdirectories in scripts not supported")),
                DirEntry::File(ref file) => {
                    let path = Path::from_path(file.path())
                        .ok_or_else(|| anyhow!("non utf-8 path in scripts"))?;

                    let name = path.file_name().expect("file to have a name");

                    Ok((name, file.contents()))
                }
            })
            .collect::<Result<_, _>>()?;

        for &(name, content) in &scripts {
            let dependencies = {
                str::from_utf8(content)?
                    .lines()
                    .take(3)
                    .find_map(|line| {
                        line.strip_prefix("# depends:").map(|rest| {
                            rest.split_whitespace()
                                .map(|s| Owned(s.to_owned()))
                                .collect::<Vec<_>>()
                        })
                    })
                    .ok_or_else(|| anyhow!("no dependencies found in {name}"))?
            };

            packages.insert(Owned(name.to_owned()), Owned(dependencies));

            plan.add(
                Owned(format!("add script {name}")),
                user_elevate,
                File {
                    path: Owned(user_directories.scripts.join(name)),
                    on_symlink: SymlinkHandling::Proceed,
                    on_directory: DirectoryHandling::Bail,
                    permissions: FilePermissions {
                        owner: Uid::from(user.name.clone()),
                        group: Gid::from(user.name.clone()),
                        mode: FileMode(0o755),
                    },
                    binary: false,
                    secret: false,
                    ignore_checksum_changes: false,
                    content: FileContent::Bytes(Ref(content)),
                },
            );
        }

        if let Some(dir) = files_in_dir(user_elevate, &user_directories.scripts).await? {
            for file in dir {
                let name = file.as_str();

                if !scripts.iter().any(|&(file, _content)| file == name) {
                    plan.add(
                        Owned(format!("remove unmanaged script {name}")),
                        user_elevate,
                        Remove {
                            path: Owned(user_directories.scripts.join(name)),
                            on_symlink: SymlinkHandling::Proceed,
                            on_directory: DirectoryHandling::Bail,
                            on_file: FileHandling::Proceed,
                        },
                    );
                }
            }
        }

        let bins: Vec<(&str, &[u8])> = BINS
            .entries()
            .iter()
            .map(|entry| match *entry {
                DirEntry::Dir(ref _d) => Err(anyhow!("subdirectories in bin not supported")),
                DirEntry::File(ref file) => {
                    let path = Path::from_path(file.path())
                        .ok_or_else(|| anyhow!("non utf-8 path in bin"))?;

                    let name = path.file_name().expect("file to have a name");

                    Ok((name, file.contents()))
                }
            })
            .collect::<Result<_, _>>()?;

        for &(name, content) in &bins {
            let dependencies = {
                str::from_utf8(content)?
                    .lines()
                    .take(3)
                    .find_map(|line| {
                        line.strip_prefix("# depends:").map(|rest| {
                            rest.split_whitespace()
                                .map(|s| Owned(s.to_owned()))
                                .collect::<Vec<_>>()
                        })
                    })
                    .ok_or_else(|| anyhow!("no dependencies found in {name}"))?
            };

            packages.insert(Owned(name.to_owned()), Owned(dependencies));

            plan.add(
                Owned(format!("add executable {name}")),
                user_elevate,
                File {
                    path: Owned(user_directories.bin.join(name)),
                    on_symlink: SymlinkHandling::Proceed,
                    on_directory: DirectoryHandling::Bail,
                    permissions: FilePermissions {
                        owner: Uid::from(user.name.clone()),
                        group: Gid::from(user.name.clone()),
                        mode: FileMode(0o755),
                    },
                    binary: false,
                    secret: false,
                    ignore_checksum_changes: false,
                    content: FileContent::Bytes(Ref(content)),
                },
            );
        }

        if let Some(dir) = files_in_dir(user_elevate, &user_directories.bin).await? {
            for file in dir {
                let name = file.as_str();

                if !bins.iter().any(|&(file, _content)| file == name) {
                    plan.add(
                        Owned(format!("remove unmanaged executable {name}")),
                        user_elevate,
                        Remove {
                            path: Owned(user_directories.bin.join(name)),
                            on_symlink: SymlinkHandling::Proceed,
                            on_directory: DirectoryHandling::Bail,
                            on_file: FileHandling::Proceed,
                        },
                    );
                }
            }
        }

        Ok(packages)
    })
    .await?
    .value_ok()
}

pub(crate) fn wallpaper<'elevate: 'action, 'action, 'user, 'machine>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    user_elevate: &'elevate Elevate,
    user: &'user UserConfig,
    user_directories: &UserDirectories,
) {
    plan.add(
        Ref("install wallpaper"),
        user_elevate,
        File {
            path: Owned(user_directories.data.join("wallpaper.jpg")),
            on_symlink: SymlinkHandling::Proceed,
            on_directory: DirectoryHandling::Bail,
            permissions: FilePermissions {
                owner: Uid::from(user.name.clone()),
                group: Gid::from(user.name.clone()),
                mode: FileMode(0o644),
            },
            binary: true,
            secret: false,
            ignore_checksum_changes: false,
            content: FileContent::Bytes(Ref(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../assets/wallpaper.jpg"
            )))),
        },
    );
}

pub(crate) fn user_services<'elevate: 'action, 'action, 'user, 'machine>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    user_elevate: &'elevate Elevate,
    user: &'user UserConfig,
    user_directories: &UserDirectories,
    services: &[MaybeOwned<'static, str>],
) {
    plan.block("services", |plan| {
        plan.block("targets", |plan| {
            plan.add(
                Ref("windowmanager.target"),
                user_elevate,
                File {
                    path: Owned(
                        user_directories
                            .systemd_user_units
                            .join("windowmanager.target"),
                    ),
                    on_symlink: SymlinkHandling::Proceed,
                    on_directory: DirectoryHandling::Bail,
                    permissions: FilePermissions {
                        owner: Uid::from(user.name.clone()),
                        group: Gid::from(user.name.clone()),
                        mode: FileMode(0o644),
                    },
                    binary: false,
                    secret: false,
                    ignore_checksum_changes: false,
                    content: FileContent::Bytes(Ref(concat!(
                        dedent!(
                            r#"
                            [Unit]
                            Requires=sway.service
                            After=sway.service
                        "#
                        ),
                        "\n"
                    )
                    .as_bytes())),
                },
            );

            plan.add(
                "autostart target",
                user_elevate,
                File {
                    path: Owned(user_directories.systemd_user_units.join("autostart.target")),
                    on_symlink: SymlinkHandling::Proceed,
                    on_directory: DirectoryHandling::Bail,
                    permissions: FilePermissions {
                        owner: Uid::from(user.name.clone()),
                        group: Gid::from(user.name.clone()),
                        mode: FileMode(0o644),
                    },
                    binary: false,
                    secret: false,
                    ignore_checksum_changes: false,
                    content: FileContent::Lines(Owned({
                        [
                            Ref("[Unit]"),
                            Ref("BindsTo=windowmanager.target"),
                            Ref("After=windowmanager.target"),
                        ]
                        .into_iter()
                        .chain(services.iter().map(|service| {
                            Owned(format!("Wants={service}", service = service.as_ref()))
                        }))
                        .collect()
                    })),
                },
            );
        });
    });
}

#[expect(clippy::implicit_hasher, reason = "does not matter")]
pub fn packages<'elevate, 'action: 'pkgs, 'user, 'machine, 'pkgs>(
    plan: &mut Block<'_, 'elevate, 'pkgs, 'action, '_, Secrets>,
    root: &'elevate Elevate,
    packages: &HashMap<MaybeOwned<'action, str>, MaybeOwned<'action, [MaybeOwned<'action, str>]>>,
) {
    plan.block("user packages", |plan| {
        #[expect(clippy::iter_over_hash_type, reason = "order does not matter")]
        for (section, packages) in packages {
            plan.add(
                Owned(format!(
                    "install user packages for {section}",
                    section = section.as_ref()
                )),
                root,
                Packages {
                    install: packages.clone(),
                    reason: InstallReason::Explicit,
                },
            );

            plan.register_packages(packages.as_ref());
        }
    });
}
