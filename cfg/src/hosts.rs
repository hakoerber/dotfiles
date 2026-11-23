use clap::ValueEnum;
use dedent::dedent;

use crate::{
    language::Language,
    root,
    secret::Secrets,
    sway::{WorkspaceApp, WorkspaceConfig},
};

use maybeowned::prelude::*;
use oxidizer::prelude::*;

#[derive(Clone, Copy, ValueEnum)]
pub enum Host {
    Neptune,
    Ares,
    Dionysus,
    Hera,
}

impl Host {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Neptune => "neptune",
            Self::Ares => "ares",
            Self::Dionysus => "dionysus",
            Self::Hera => "hera",
        }
    }

    pub fn machine_config(self) -> MachineConfig {
        match self {
            Self::Neptune => MachineConfig {
                host: self,
                encrypted_root: true,
                boot_target: BootTarget::MultiUser,
                cpu: Cpu::Intel,
                gpu: Gpu::Nvidia,
                font_size: 11,
                machine_type: MachineType::Laptop,
                roles: MachineRoles(&[]),
                language: Language::En,
            },
            Self::Ares => MachineConfig {
                host: self,
                encrypted_root: true,
                boot_target: BootTarget::MultiUser,
                cpu: Cpu::Amd,
                gpu: Gpu::Amd,
                font_size: 10,
                machine_type: MachineType::Workstation,
                roles: MachineRoles(&[MachineRole::Gaming]),
                language: Language::En,
            },
            Self::Hera => MachineConfig {
                host: self,
                encrypted_root: false,
                boot_target: BootTarget::Graphical,
                cpu: Cpu::Intel,
                gpu: Gpu::Intel,
                font_size: 11,
                machine_type: MachineType::Laptop,
                roles: MachineRoles(&[]),
                language: Language::De,
            },
            Self::Dionysus => MachineConfig {
                host: self,
                encrypted_root: true,
                boot_target: BootTarget::MultiUser,
                cpu: Cpu::Intel,
                gpu: Gpu::Intel,
                font_size: 16,
                machine_type: MachineType::Tv,
                roles: MachineRoles(&[]),
                language: Language::En,
            },
        }
    }

    pub fn workspace_config(self) -> WorkspaceConfig {
        match self {
            Self::Neptune => WorkspaceConfig {
                num1: Some(WorkspaceApp::Firefox),
                num2: Some(WorkspaceApp::Terminal),
                num3: Some(WorkspaceApp::Terminal),
                num4: Some(WorkspaceApp::Terminal),
                num5: None,
                num6: None,
                num7: Some(WorkspaceApp::Meet),
                num8: Some(WorkspaceApp::Keepass),
                num9: Some(WorkspaceApp::Spotify),
                num0: Some(WorkspaceApp::Pim),
            },
            Self::Ares => WorkspaceConfig {
                num1: Some(WorkspaceApp::Firefox),
                num2: Some(WorkspaceApp::Terminal),
                num3: Some(WorkspaceApp::Terminal),
                num4: Some(WorkspaceApp::Terminal),
                num5: None,
                num6: None,
                num7: Some(WorkspaceApp::Spotify),
                num8: Some(WorkspaceApp::Discord),
                num9: Some(WorkspaceApp::Game),
                num0: Some(WorkspaceApp::Steam),
            },
            Self::Hera => WorkspaceConfig {
                num1: Some(WorkspaceApp::Firefox),
                num2: Some(WorkspaceApp::Terminal),
                num3: Some(WorkspaceApp::Terminal),
                num4: Some(WorkspaceApp::Terminal),
                num5: None,
                num6: None,
                num7: None,
                num8: None,
                num9: None,
                num0: Some(WorkspaceApp::Spotify),
            },
            Self::Dionysus => WorkspaceConfig {
                num1: Some(WorkspaceApp::Tv),
                num2: Some(WorkspaceApp::Firefox),
                num3: Some(WorkspaceApp::Spotify),
                num4: None,
                num5: None,
                num6: None,
                num7: None,
                num8: None,
                num9: None,
                num0: None,
            },
        }
    }

    pub fn specials<'elevate: 'action, 'action>(
        self,
        plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
        root: &'elevate Elevate,
    ) {
        match self {
            Self::Hera => {
                plan.block("hera", |plan| {
                    let packages = &[
                        Ref("plasma-desktop"),
                        Ref("konsole"),
                        Ref("dolphin"),
                        Ref("kdeplasma-addons"),
                        Ref("plasma-nm"),
                        Ref("plasma-pa"),
                        Ref("plasma-systemmonitor"),
                        Ref("plasma-login-manager"),
                        Ref("thunderbird"),
                        // kde archive manager
                        Ref("ark"),
                        // kde image viewer
                        Ref("gwenview"),
                        // german language packs
                        Ref("hunspell-de"),
                        Ref("thunderbird-i18n-de"),
                        Ref("firefox-i18n-de"),
                    ];

                    plan.add(
                        "install packages",
                        root,
                        Packages {
                            install: Ref(packages),
                            reason: InstallReason::Explicit,
                        },
                    );

                    plan.register_packages(packages);

                    plan.add(
                        Ref("create user"),
                        root,
                        User {
                            name: "herta",
                            home: Ref(Path::new("/home/herta")),
                            primary_group: PrimaryGroupConfig::Automatic,
                            additional_groups: &["wheel", "sudonopw", "games", "kvm", "video"],
                            shell: "/usr/bin/zsh",
                            system: false,
                        },
                    );

                    plan.block("login manager", |plan| {
                        plan.add(
                            Ref("enable plasma-login-manager"),
                            root,
                            Service {
                                name: "plasmalogin.service",
                                state: ServiceState {
                                    runtime: ServiceStateRuntime::Active,
                                    persistent: ServiceStatePersistent::Enabled,
                                },
                            },
                        );

                        plan.add(
                            "configure plasma login manager",
                            root,
                            File {
                                path: Ref(Path::new("/etc/plasmalogin.conf")),
                                permissions: FilePermissions {
                                    owner: Uid::root(),
                                    group: Gid::root(),
                                    mode: FileMode(0o644),
                                },
                                on_symlink: SymlinkHandling::Proceed,
                                on_directory: DirectoryHandling::Proceed,
                                binary: false,
                                secret: false,
                                ignore_checksum_changes: false,
                                content: FileContent::Bytes(Ref(dedent!(
                                    r#"
                                        [Autologin]
                                        User=herta
                                        Session=plasma.desktop
                                    "#
                                )
                                .as_bytes())),
                            },
                        );

                        plan.add(
                            "lock on startup",
                            root,
                            File {
                                path: Ref(Path::new("/etc/xdg/kscreenlockerrc")),
                                permissions: FilePermissions {
                                    owner: Uid::root(),
                                    group: Gid::root(),
                                    mode: FileMode(0o644),
                                },
                                on_symlink: SymlinkHandling::Proceed,
                                on_directory: DirectoryHandling::Proceed,
                                binary: false,
                                secret: false,
                                ignore_checksum_changes: false,
                                #[expect(
                                    clippy::string_lit_as_bytes,
                                    reason = "false positive due to the dedent!() macro"
                                )]
                                content: FileContent::Bytes(Ref(dedent!(
                                    r#"
                                        [Daemon]
                                        LockOnStart=true
                                    "#
                                )
                                .as_bytes())),
                            },
                        );
                    });

                    root::autoupdate(plan, root);
                    root::restic_backup(plan, root);
                });
            }
            Self::Dionysus => {
                plan.block("dionysus", |plan| {
                    root::autoupdate(plan, root);
                });
            }
            Self::Ares | Self::Neptune => (),
        }
    }

    pub fn bootstrap_script(self) -> &'static [u8] {
        match self {
            Self::Neptune => include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../install_scripts/neptune.sh"
            )),
            Self::Ares => include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../install_scripts/ares.sh"
            )),
            Self::Dionysus => include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../install_scripts/dionysus.sh"
            )),
            Self::Hera => include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../install_scripts/hera.sh"
            )),
        }
    }
}

pub enum Cpu {
    Intel,
    Amd,
}

pub enum Gpu {
    Amd,
    Nvidia,
    Intel,
}

#[derive(Eq, PartialEq, Clone, Copy)]
pub enum MachineType {
    Tv,
    Workstation,
    Laptop,
}

impl MachineType {
    pub(crate) fn is_tv(self) -> bool {
        self == Self::Tv
    }

    pub(crate) fn is_workstation(self) -> bool {
        self == Self::Workstation
    }

    pub(crate) fn is_laptop(self) -> bool {
        self == Self::Laptop
    }
}

pub struct MachineRoles(pub &'static [MachineRole]);

impl MachineRoles {
    fn has(&self, role: MachineRole) -> bool {
        self.0.contains(&role)
    }

    pub(crate) fn has_gaming(&self) -> bool {
        self.has(MachineRole::Gaming)
    }
}

#[derive(Eq, PartialEq, Clone, Copy)]
pub enum MachineRole {
    Gaming,
}

pub struct MachineConfig {
    pub host: Host,
    pub encrypted_root: bool,
    pub boot_target: BootTarget,
    pub cpu: Cpu,
    pub gpu: Gpu,
    pub font_size: usize,
    pub machine_type: MachineType,
    pub roles: MachineRoles,
    pub language: Language,
}

impl MachineConfig {
    fn private_user(autologin: bool, vt: usize, name: String, machine: &Self) -> UserConfig {
        UserConfig {
            autologin,
            vt,
            name,
            fullname: "Hannes Körber".to_owned(),
            mail: "hannes@hkoerber.de".to_owned(),
            keyboard: if machine.machine_type == MachineType::Tv {
                UserKeyboardConfig {
                    repeat_delay: 1000,
                    repeat_rate: 10,
                }
            } else {
                UserKeyboardConfig {
                    repeat_delay: 150,
                    repeat_rate: 50,
                }
            },
            ssh_agent_enabled: false,
            gpg: Some(UserGpgConfig {
                agent_enabled: true,
                gpg_agent_for_ssh: true,
            }),
            nextcloud: FeatureState::Enabled,
            steam: machine.roles.has(MachineRole::Gaming).into(),
            keepassxc: FeatureState::Disabled,
            firefox: FirefoxUserConfig {
                profiles: vec![FirefoxProfile {
                    name: "default".to_owned(),
                    manage_css: true,
                }],
            },
            workspace: machine.host.workspace_config(),
        }
    }

    pub fn with_users(self) -> RawConfig {
        let work_user = UserConfig {
            autologin: true,
            vt: 1,
            name: "hannes-work".to_owned(),
            fullname: "Hannes Körber".to_owned(),
            mail: "h.koerber@clipmyhorse.tv".to_owned(),
            keyboard: UserKeyboardConfig {
                repeat_delay: 150,
                repeat_rate: 50,
            },
            ssh_agent_enabled: true,
            gpg: None,
            nextcloud: FeatureState::Disabled,
            steam: FeatureState::Disabled,
            keepassxc: FeatureState::Enabled,
            firefox: FirefoxUserConfig {
                profiles: vec![
                    FirefoxProfile {
                        name: "default".to_owned(),
                        manage_css: true,
                    },
                    FirefoxProfile {
                        name: "meet".to_owned(),
                        manage_css: false,
                    },
                ],
            },
            workspace: WorkspaceConfig {
                num1: Some(WorkspaceApp::Firefox),
                num2: Some(WorkspaceApp::Terminal),
                num3: Some(WorkspaceApp::Terminal),
                num4: Some(WorkspaceApp::Terminal),
                num5: None,
                num6: None,
                num7: Some(WorkspaceApp::Meet),
                num8: Some(WorkspaceApp::Keepass),
                num9: Some(WorkspaceApp::Spotify),
                num0: Some(WorkspaceApp::Pim),
            },
        };

        let users = match self.host {
            Host::Neptune => vec![
                Self::private_user(true, 2, "hannes-private".to_owned(), &self),
                work_user,
            ],
            Host::Ares => vec![Self::private_user(true, 1, "hannes".to_owned(), &self)],
            Host::Dionysus => vec![Self::private_user(false, 1, "hannes".to_owned(), &self)],
            Host::Hera => vec![Self::private_user(false, 2, "hannes".to_owned(), &self)],
        };

        RawConfig {
            machine: self,
            users,
        }
    }
}

pub struct RawConfig {
    machine: MachineConfig,
    users: Vec<UserConfig>,
}

impl RawConfig {
    pub fn with_running_user(self, running_user: &str) -> Config {
        let users = self
            .users
            .into_iter()
            .map(|config| UserContext {
                simplest_elevate: match running_user {
                    "root" => Elevate::No,
                    name if name == config.name => Elevate::No,
                    _other => Elevate::Sudo(ElevateTo::User {
                        user: config.name.clone(),
                    }),
                },
                strict_elevate: match running_user {
                    name if name == config.name => Elevate::No,
                    _other => Elevate::Sudo(ElevateTo::User {
                        user: config.name.clone(),
                    }),
                },
                config,
            })
            .collect();

        Config {
            users,
            machine: self.machine,
        }
    }
}

pub struct UserConfig {
    pub name: String,
    pub fullname: String,
    pub mail: String,
    pub keyboard: UserKeyboardConfig,
    pub ssh_agent_enabled: bool,
    pub gpg: Option<UserGpgConfig>,
    pub vt: usize,
    pub nextcloud: FeatureState,
    pub steam: FeatureState,
    pub keepassxc: FeatureState,
    pub autologin: bool,
    pub firefox: FirefoxUserConfig,
    pub workspace: WorkspaceConfig,
}

pub struct UserKeyboardConfig {
    pub repeat_delay: usize,
    pub repeat_rate: usize,
}

pub struct UserGpgConfig {
    pub gpg_agent_for_ssh: bool,
    pub agent_enabled: bool,
}

pub struct FirefoxProfile {
    pub name: String,
    pub manage_css: bool,
}

pub struct FirefoxUserConfig {
    pub profiles: Vec<FirefoxProfile>,
}

#[derive(PartialEq, Eq)]
pub enum FeatureState {
    Enabled,
    Disabled,
}

impl FeatureState {
    pub fn enabled(&self) -> bool {
        *self == Self::Enabled
    }
}

impl From<bool> for FeatureState {
    fn from(value: bool) -> Self {
        if value { Self::Enabled } else { Self::Disabled }
    }
}

pub struct UserContext {
    pub simplest_elevate: Elevate,
    pub strict_elevate: Elevate,
    pub config: UserConfig,
}

pub struct Config {
    pub machine: MachineConfig,
    pub users: Vec<UserContext>,
}
