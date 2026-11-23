use std::collections::HashSet;

use anyhow::bail;

use anyhow::Result;
use dedent::dedent;
use strum::VariantArray as _;

use maybeowned::prelude::*;
use oxidizer::prelude::*;

use crate::{
    hosts::{Config, Cpu, Gpu},
    language::Language,
    secret::Secrets,
};

pub fn autoupdate<'elevate: 'action, 'action, 'machine>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.block("autoupdate", |plan| {
        plan.add(
            "deploy autoupdate script",
            elevate,
            File {
                path: Ref(Path::new("/usr/local/bin/pacman-autoupdate")),
                permissions: FilePermissions {
                    owner: Uid::root(),
                    group: Gid::root(),
                    mode: FileMode(0o755),
                },
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Proceed,
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(
                    dedent!(r#"
                        #!/usr/bin/env bash

                        set -o errexit
                        set -o nounset
                        set -o pipefail

                        # Prevent failures when no battery present
                        shopt -s nullglob

                        for battery in /sys/class/power_supply/*/capacity ; do
                            capacity="$(< "$battery")"
                            if (( "${capacity}" < 40 )) ; then
                                printf "Battery at %s%%, exiting\n" "${capacity}" >&2
                                exit 0
                            fi
                        done

                        if nmcli --terse --fields GENERAL.METERED dev show 2>/dev/null | grep -q "yes" ; then
                            printf "Detected metered connection, exiting\n" >&2
                            exit 0
                        fi

                        # Make sure that keys are up to date, otherwise sig checks may fail
                        pacman --sync --noprogressbar --noconfirm --refresh --needed archlinux-keyring

                        pacman --sync --noprogressbar --noconfirm --sysupgrade
                    "#)
                    .as_bytes(),
                )),
            },
        );

        plan.add(
            "install pacman autoupdate service",
            elevate,
            File {
                path: Ref(Path::new("/etc/systemd/system/pacman-autoupdate.service")),
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
                        [Service]
                        Type=oneshot
                        ExecStart=/usr/local/bin/pacman-autoupdate
                    "#
                )
                .as_bytes())),
            },
        )
        .on_change(|| {
            Resource::new(
                Ref("restart service manager due to new packages"),
                elevate,
                DaemonReload {},
            )
        });

        plan.add(
            "install pacman autoupdate timer",
            elevate,
            File {
                path: Ref(Path::new("/etc/systemd/system/pacman-autoupdate.timer")),
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
                        [Timer]
                        OnCalendar=daily
                        OnBootSec=5min
                        OnUnitInactiveSec=120min

                        [Install]
                        WantedBy=timers.target
                    "#
                )
                .as_bytes())),
            },
        )
        .on_change(|| {
            Resource::new(
                Ref("restart service manager due to new packages"),
                elevate,
                DaemonReload {},
            )
        });

        plan.add(
            Ref("enable pacman autoupdate timer"),
            elevate,
            Service {
                name: "pacman-autoupdate.timer",
                state: ServiceState {
                    runtime: ServiceStateRuntime::Active,
                    persistent: ServiceStatePersistent::Enabled,
                },
            },
        );
    });
}

pub fn fstrim<'elevate: 'action, 'action, 'machine>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    let util_linux = &[Ref("util-linux")];

    plan.block("ssd trim", |plan| {
        plan.add(
            "install linux-utils",
            elevate,
            Packages {
                install: Ref(util_linux),
                reason: InstallReason::Explicit,
            },
        );

        plan.register_packages(util_linux);

        plan.add(
            Ref("enable fstrim timer"),
            elevate,
            Service {
                name: "fstrim.timer",
                state: ServiceState {
                    runtime: ServiceStateRuntime::Active,
                    persistent: ServiceStatePersistent::Enabled,
                },
            },
        );
    });
}

pub fn restic_backup<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.block("restic", |plan| {
        let restic = &[Ref("restic")];

        plan.add(
            "install restic",
            elevate,
            Packages {
                install: Ref(restic),
                reason: InstallReason::Explicit,
            },
        );

        plan.register_packages(restic);

        plan.add(
            Ref("create restic config directory"),
            elevate,
            Directory {
                path: Ref(Path::new("/etc/restic")),
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
            Ref("create restic cache directory"),
            elevate,
            Directory {
                path: Ref(Path::new("/var/cache/restic")),
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
            "create restic exclude file",
            elevate,
            File {
                path: Ref(Path::new("/etc/restic/exclude.lst")),
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
                        /home/*/.cache/**
                        /home/*/.mozilla/firefox/*/Cache/**
                    "#
                )
                .as_bytes())),
            },
        );

        plan.add(
            "create restic wrapper script",
            elevate,
            File {
                path: Ref(Path::new("/usr/local/bin/restic-cmd")),
                permissions: FilePermissions {
                    owner: Uid::root(),
                    group: Gid::root(),
                    mode: FileMode(0o755),
                },
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Proceed,
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(dedent!(
                    r#"
                        #!/usr/bin/env bash
                        source /etc/restic/env

                        set -o nounset
                        set -o errexit
                        set -o pipefail

                        export B2_ACCOUNT_ID
                        export B2_ACCOUNT_KEY

                        export RESTIC_PASSWORD_FILE=/etc/restic/repopassword

                        restic \
                          --cache-dir=/var/cache/restic/ \
                          --repo="b2:${BUCKET_NAME}:hera" \
                          --password-file=/etc/restic/repopassword \
                          --verbose \
                          "${@}"
                    "#
                )
                .as_bytes())),
            },
        );

        plan.add(
            "create restic backup script",
            elevate,
            File {
                path: Ref(Path::new("/usr/local/bin/restic-backup")),
                permissions: FilePermissions {
                    owner: Uid::root(),
                    group: Gid::root(),
                    mode: FileMode(0o755),
                },
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Proceed,
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(dedent!(
                    r#"
                        #!/usr/bin/env bash

                        set -o nounset
                        set -o errexit
                        set -o pipefail

                        run() {
                          name="${1}" ; shift
                          printf '[%s] %s - start\n' "${name}" "$(date --utc --iso-8601=seconds)"
                          "${@}"
                          printf '[%s] %s - end\n' "${name}" "$(date --utc --iso-8601=seconds)"
                        }

                        run backup restic-cmd \
                            backup \
                            --exclude-file /etc/restic/exclude.lst \
                            /home/

                        run forget restic-cmd \
                            forget \
                            --prune \
                            --keep-daily 30 \
                            --keep-monthly 12 \
                            --keep-yearly 3
                    "#
                )
                .as_bytes())),
            },
        );

        plan.block("restic backup service", |block| {
            block.with(elevate, |mut block| {
                block.add(
                    "install restic backup service",
                    File {
                        path: Ref(Path::new("/etc/systemd/system/restic-backup.service")),
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
                                [Service]
                                Type=oneshot
                                ExecStart=systemd-inhibit /usr/local/bin/restic-backup
                            "#
                        )
                        .as_bytes())),
                    },
                );

                block.add(
                    "install restic backup timer",
                    File {
                        path: Ref(Path::new("/etc/systemd/system/restic-backup.timer")),
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
                                [Timer]
                                OnCalendar=daily
                                Persistent=true

                                [Install]
                                WantedBy=timers.target
                            "#
                        )
                        .as_bytes())),
                    },
                );
            });
        })
        .on_change(|| {
            Resource::new(
                Ref("restart service manager due to new packages"),
                elevate,
                DaemonReload {},
            )
        });

        plan.add(
            Ref("enable pacman autoupdate timer"),
            elevate,
            Service {
                name: "restic-backup.timer",
                state: ServiceState {
                    runtime: ServiceStateRuntime::Active,
                    persistent: ServiceStatePersistent::Enabled,
                },
            },
        );
    });
}

pub fn initrd<'name, 'elevate: 'action, 'action>(
    plan: &mut Block<'name, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
    config: &Config,
) {
    plan.block("initrd", |plan| {
        let mkinitcpio = &[Ref("mkinitcpio")];
        plan.add(
            "install mkinitcpio",
            elevate,
            Packages {
                install: Ref(mkinitcpio),
                reason: InstallReason::Explicit,
            },
        );
        plan.register_packages(mkinitcpio);

        let lz4 = &[Ref("lz4")];
        plan.add(
            "install lz4",
            elevate,
            Packages {
                install: Ref(lz4),
                reason: InstallReason::Explicit,
            },
        );

        plan.register_packages(lz4);

        let hooks: Vec<&str> = {
            let mut hooks = vec![
                "base",
                "udev",
                "autodetect",
                "microcode",
                "modconf",
                "kms",
                "keyboard",
                "keymap",
                "consolefont",
                "block",
            ];

            if config.machine.encrypted_root {
                hooks.extend(["encrypt", "lvm2"]);
            }

            hooks.extend(["filesystems", "resume", "fsck"]);

            hooks
        };

        let rebuild_mkinitcpio = Command {
            cmd: Ref("mkinitcpio"),
            args: &["-P"],
            check_command: None,
        };

        plan.add(
            "configure mkinitcpio",
            elevate,
            File {
                path: Ref(Path::new("/etc/mkinitcpio.conf")),
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
                content: FileContent::Bytes(Owned(
                    format!(
                        dedent!(
                            r#"
                                MODULES=()
                                BINARIES=()
                                FILES=()
                                HOOKS=({hooks})
                                COMPRESSION="lz4"
                            "#
                        ),
                        hooks = hooks.join(" ")
                    )
                    .into_bytes(),
                )),
            },
        )
        .on_change(|| Resource::new(Ref("rebuild initrd"), elevate, rebuild_mkinitcpio));
    });
}

pub fn libvirt<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.block("libvirt", |plan| {
        let libvirt = &[
            Ref("virt-manager"),
            Ref("libvirt"),
            Ref("dnsmasq"),
            Ref("dmidecode"),
            Ref("virt-install"),
            Ref("virt-viewer"),
            Ref("libguestfs"),
            Ref("edk2-ovmf"),
        ];
        plan.register_packages(libvirt);

        plan.add(
            "install libvirt",
            elevate,
            Packages {
                install: Ref(libvirt),
                reason: InstallReason::Explicit,
            },
        );

        for socket in ["libvirtd.socket", "virtlogd.socket"] {
            plan.add(
                Owned(format!("enable {socket}")),
                elevate,
                Service {
                    name: socket,
                    state: ServiceState {
                        runtime: ServiceStateRuntime::Active,
                        persistent: ServiceStatePersistent::Enabled,
                    },
                },
            );
        }
    });
}

pub fn docker<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.block("docker", |plan| {
        let docker = &[Ref("docker"), Ref("docker-compose")];
        plan.register_packages(docker);

        plan.add(
            "install docker",
            elevate,
            Packages {
                install: Ref(docker),
                reason: InstallReason::Explicit,
            },
        );

        plan.add(
            Ref("enable docker socket"),
            elevate,
            Service {
                name: "docker.socket",
                state: ServiceState {
                    runtime: ServiceStateRuntime::Active,
                    persistent: ServiceStatePersistent::Enabled,
                },
            },
        );
    });
}

pub fn wireshark<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.block("wireshark", |plan| {
        let wireshark = &[Ref("wireshark-cli"), Ref("wireshark-qt")];
        plan.add(
            "install wireshark",
            elevate,
            Packages {
                install: Ref(wireshark),
                reason: InstallReason::Explicit,
            },
        );
        plan.register_packages(wireshark);
    });
}

pub fn smartcard<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.block("smartcard", |plan| {
        let smartcard = &[Ref("ccid"), Ref("pcsclite")];
        plan.add(
            "install smartcard manager",
            elevate,
            Packages {
                install: Ref(smartcard),
                reason: InstallReason::Explicit,
            },
        );
        plan.register_packages(smartcard);

        plan.add(
            "enable smartcard daemon socket",
            elevate,
            Service {
                name: "pcscd.socket",
                state: ServiceState {
                    runtime: ServiceStateRuntime::Active,
                    persistent: ServiceStatePersistent::Enabled,
                },
            },
        );
    });
}

pub fn boot_target<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
    config: &Config,
) {
    plan.add(
        Ref("setting boot target"),
        elevate,
        config.machine.boot_target,
    );
}

pub fn sudo<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.block("sudo", |plan| {
        let sudo = &[Ref("sudo")];

        plan.add(
            "install sudo",
            elevate,
            Packages {
                install: Ref(sudo),
                reason: InstallReason::Explicit,
            },
        );

        plan.register_packages(sudo);

        plan.add(
            Ref("add sudonopw"),
            elevate,
            Group {
                name: "sudonopw",
                system: true,
            },
        );

        plan.add(
            Ref("configure passwordless sudo"),
            elevate,
            File {
                path: Ref(Path::new("/etc/sudoers.d/sudonopw")),
                permissions: FilePermissions {
                    owner: Uid::root(),
                    group: Gid::root(),
                    mode: FileMode(0o600),
                },
                on_directory: DirectoryHandling::Bail,
                on_symlink: SymlinkHandling::Proceed,
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Lines(Owned(vec![Ref("%sudonopw ALL=(ALL) NOPASSWD: ALL")])),
            },
        );

        let doas = &[Ref("opendoas")];
        plan.add(
            "install doas",
            elevate,
            Packages {
                install: Ref(doas),
                reason: InstallReason::Explicit,
            },
        );
        plan.register_packages(doas);

        plan.add(
            Ref("configure passwordless doas"),
            elevate,
            File {
                path: Ref(Path::new("/etc/doas.conf")),
                permissions: FilePermissions {
                    owner: Uid::root(),
                    group: Gid::root(),
                    mode: FileMode(0o400),
                },
                on_directory: DirectoryHandling::Bail,
                on_symlink: SymlinkHandling::Proceed,
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Lines(Owned(vec![Ref("permit nopass nolog setenv {PATH=/usr/local/bin:/usr/local/sbin:/usr/bin:/usr/sbin} :sudonopw")])),
            },
        );
    });
}

pub fn timesyncd<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.block("timesyncd", |plan| {
        let arch_ntp_servers = [
            "0.arch.pool.ntp.org",
            "1.arch.pool.ntp.org",
            "2.arch.pool.ntp.org",
            "3.arch.pool.ntp.org",
        ];

        let fallback_ntp_servers = [
            "0.pool.ntp.org",
            "1.pool.ntp.org",
            "2.pool.ntp.org",
            "3.pool.ntp.org",
        ];

        plan.add(
            Ref("configure timesyncd"),
            elevate,
            File {
                path: Ref(Path::new("/etc/systemd/timesyncd.conf")),
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
                    Ref("[Time]"),
                    Owned(format!("NTP={}", arch_ntp_servers.join(" "))),
                    Owned(format!("FallbackNTP={}", fallback_ntp_servers.join(" "))),
                ])),
            },
        );

        plan.add(
            "enable timesyncd",
            elevate,
            Service {
                name: "systemd-timesyncd.service",
                state: ServiceState {
                    runtime: ServiceStateRuntime::Active,
                    persistent: ServiceStatePersistent::Enabled,
                },
            },
        );
    });
}

pub fn reflector<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.block("reflector", |plan| {
        let reflector = &[Ref("reflector")];
        plan.add(
            "install reflector",
            elevate,
            Packages {
                install: Ref(reflector),
                reason: InstallReason::Explicit,
            },
        );
        plan.register_packages(reflector);

        plan.add(
            "configure reflector",
            elevate,
            File {
                path: Ref(Path::new("/etc/xdg/reflector/reflector.conf")),
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
                content: FileContent::Lines(Ref(&[
                    Ref("--save /etc/pacman.d/mirrorlist"),
                    Ref("--protocol https"),
                    Ref("--country Germany"),
                    Ref("--latest 5"),
                    Ref("--sort age"),
                ])),
            },
        )
        .on_change(|| {
            Resource::new(
                Ref("restart reflector"),
                elevate,
                RestartService {
                    name: "reflector.service",
                },
            )
        });

        plan.add(
            "enable reflector timer",
            elevate,
            Service {
                name: "reflector.timer",
                state: ServiceState {
                    runtime: ServiceStateRuntime::Active,
                    persistent: ServiceStatePersistent::Enabled,
                },
            },
        );
    });
}

pub fn wireguard<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) -> Result<()> {
    plan.try_block("wireguard", |plan| {
        let wireguard = &[Ref("wireguard-tools")];
        plan.add(
            "install wireguard",
            elevate,
            Packages {
                install: Ref(wireguard),
                reason: InstallReason::Explicit,
            },
        );
        plan.register_packages(wireguard);

        let path = Path::new("/etc/wireguard/wg0.conf");
        let permissions = FilePermissions {
            owner: Uid::root(),
            group: Gid::root(),
            mode: FileMode(0o600),
        };
        match plan.secrets() {
            SecretsWorkflow::None => bail!("missing secrets"),
            SecretsWorkflow::AssumeExisting => plan.assert(
                "assert wireguard config exists",
                elevate,
                AssertFile {
                    path: Ref(path),
                    permissions,
                },
            ),
            SecretsWorkflow::Secrets(secrets) => {
                let mut context = tera::Context::new();
                context.insert("address", &secrets.wireguard.address.access());
                context.insert("privkey", &secrets.wireguard.privkey.access().as_str());
                context.insert("psk", &secrets.wireguard.psk.access().as_str());
                context.insert(
                    "server_pubkey",
                    &secrets.wireguard.server_pubkey.access().as_str(),
                );
                context.insert("server_ip", &secrets.wireguard.server_ip.access());
                context.insert(
                    "server_endpoint_domain",
                    &secrets.wireguard.server_endpoint_domain.access().as_str(),
                );
                context.insert(
                    "server_endpoint_port",
                    &secrets.wireguard.server_endpoint_port.access().as_inner(),
                );
                let template = tera::Tera::one_off(
                    &dedent!(
                        r#"
                        [Interface]
                        Address = {{address}}/32
                        PrivateKey = {{privkey}}
                        MTU = 1300

                        [Peer]
                        PublicKey = {{server_pubkey}}
                        PresharedKey = {{psk}}
                        AllowedIPs = {{server_ip}}/24
                        Endpoint = {{server_endpoint_domain}}:{{server_endpoint_port}}
                        PersistentKeepalive = 25
                    "#
                    ),
                    &context,
                    false,
                )?;

                plan.add(
                    "configure wireguard",
                    elevate,
                    File {
                        path: Ref(path),
                        permissions,
                        on_directory: DirectoryHandling::Bail,
                        on_symlink: SymlinkHandling::Bail,
                        binary: false,
                        secret: true,
                        ignore_checksum_changes: false,
                        content: FileContent::Bytes(Owned(template.into_bytes())),
                    },
                )
                .on_change(|| {
                    Resource::new(
                        Ref("restart wireguard"),
                        elevate,
                        RestartService {
                            name: "wg-quick@wg0.service",
                        },
                    )
                });
            }
        }

        plan.add(
            "enable wireguard tunnel",
            elevate,
            Service {
                name: "wg-quick@wg0.service",
                state: ServiceState {
                    runtime: ServiceStateRuntime::Active,
                    persistent: ServiceStatePersistent::Enabled,
                },
            },
        );

        plan.add(
            "add wireguard-reresolve-dns service",
            elevate,
            File {
                path: Ref(Path::new(
                    "/etc/systemd/system/wireguard-reresolve-dns.service",
                )),
                on_directory: DirectoryHandling::Bail,
                on_symlink: SymlinkHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::root(),
                    group: Gid::root(),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(dedent!(
                    "
                [Unit]
                Wants=network-online.target
                After=network-online.target nss-lookup.target

                [Service]
                Type=oneshot
                ExecStart=/usr/share/wireguard-tools/examples/reresolve-dns/reresolve-dns.sh wg0
                "
                )
                .as_bytes())),
            },
        );

        plan.add(
            "wireguard dns reresolve timer",
            elevate,
            File {
                path: Ref(Path::new(
                    "/etc/systemd/system/wireguard-reresolve-dns.timer",
                )),
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
                content: FileContent::Bytes(Ref(dedent!(
                    "
                [Timer]
                OnBootSec=1m
                OnUnitActiveSec=30s

                [Install]
                WantedBy=timers.target
                "
                )
                .as_bytes())),
            },
        );

        plan.add(
            "enable wireguard dns reresolve timer",
            elevate,
            Service {
                name: "wireguard-reresolve-dns.timer",
                state: ServiceState {
                    runtime: ServiceStateRuntime::Active,
                    persistent: ServiceStatePersistent::Enabled,
                },
            },
        );

        plan.add(
            "force networkmanager to ignore wireguard interfaces",
            elevate,
            File {
                path: Ref(Path::new("/etc/NetworkManager/conf.d/unmanaged.conf")),
                on_directory: DirectoryHandling::Bail,
                on_symlink: SymlinkHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::root(),
                    group: Gid::root(),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(dedent!(
                    "
                    [keyfile]
                    unmanaged-devices=type:wireguard
                    "
                )
                .as_bytes())),
            },
        );

        plan.add(
            Ref("create sshd override directory"),
            elevate,
            Directory {
                path: Ref(Path::new("/etc/systemd/system/sshd.service.d/")),
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
            "add systemd override for sshd wireguard dependency",
            elevate,
            File {
                path: Ref(Path::new(
                    "/etc/systemd/system/sshd.service.d/override.conf",
                )),
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
                        [Unit]
                        After=wg-quick@wg0.service
                        Requires=sys-devices-virtual-net-wg0.device
                    "#
                )
                .as_bytes())),
            },
        )
        .on_change(|| {
            Resource::new(
                Ref("restart service manager due to sshd override"),
                elevate,
                DaemonReload {},
            )
        });

        Ok(())
    })?;

    Ok(())
}

pub fn pipewire<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.block("pipewire", |plan| {
        let pipewire = &[
            Ref("pipewire"),
            Ref("lib32-pipewire"),
            Ref("wireplumber"),
            Ref("pavucontrol-qt"),
            Ref("pipewire-audio"),
            Ref("pipewire-pulse"),
            Ref("pipewire-alsa"),
            Ref("sof-firmware"),
            Ref("rtkit"),
        ];

        plan.add(
            "install pipewire",
            elevate,
            Packages {
                install: Ref(pipewire),
                reason: InstallReason::Explicit,
            },
        );

        plan.register_packages(pipewire);
    });
}

pub fn fonts<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    let fonts = &[
        Ref("awesome-terminal-fonts"),
        Ref("ttf-jetbrains-mono"),
        Ref("ttf-inconsolata-nerd"),
        Ref("ttf-dejavu"),
    ];

    plan.add(
        "install fonts",
        elevate,
        Packages {
            install: Ref(fonts),
            reason: InstallReason::Explicit,
        },
    );
    plan.register_packages(fonts);
}

pub fn sshd<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) -> Result<()> {
    plan.try_block("sshd", |plan| {
        let ssh = &[Ref("openssh")];
        plan.add(
            "install ssh",
            elevate,
            Packages {
                install: Ref(ssh),
                reason: InstallReason::Explicit,
            },
        );
        plan.register_packages(ssh);

        plan.add(
            Ref("add ssh group"),
            elevate,
            Group {
                name: "ssh",
                system: true,
            },
        );

        let path = Path::new("/etc/ssh/sshd_config");
        let permissions = FilePermissions {
            owner: Uid::root(),
            group: Gid::root(),
            mode: FileMode(0o644),
        };
        match plan.secrets() {
            SecretsWorkflow::None => bail!("missing secrets"),
            SecretsWorkflow::AssumeExisting => plan.assert(
                "assert wireguard config exists",
                elevate,
                AssertFile {
                    path: Ref(path),
                    permissions,
                },
            ),
            SecretsWorkflow::Secrets(secrets) => {
                let mut context = tera::Context::new();
                context.insert("address", &secrets.wireguard.address.access());
                let template = tera::Tera::one_off(
                    &dedent!(
                        r#"
                        AddressFamily inet
                        ListenAddress 127.0.0.1
                        ListenAddress {{ address }}
                        Port 22

                        UsePAM yes

                        PrintMotd no

                        AllowGroups ssh

                        AuthenticationMethods publickey

                        AuthorizedKeysFile .ssh/authorized_keys

                        DisableForwarding yes

                        PubkeyAuthentication yes
                        PasswordAuthentication no
                        GSSAPIAuthentication no
                        HostbasedAuthentication no
                        KerberosAuthentication no
                        KbdInteractiveAuthentication no

                        PermitRootLogin no
                "#
                    ),
                    &context,
                    false,
                )?;

                plan.add(
                    "configure sshd",
                    elevate,
                    File {
                        path: Ref(path),
                        permissions,
                        on_directory: DirectoryHandling::Bail,
                        on_symlink: SymlinkHandling::Bail,
                        binary: false,
                        secret: false,
                        ignore_checksum_changes: false,
                        content: FileContent::Bytes(Owned(template.into_bytes())),
                    },
                )
                .on_change(|| {
                    Resource::new(
                        Ref("restart sshd"),
                        elevate,
                        RestartService { name: "sshd" },
                    )
                });
            }
        }

        plan.add(
            "enable sshd",
            elevate,
            Service {
                name: "sshd.service",
                state: ServiceState {
                    runtime: ServiceStateRuntime::Active,
                    persistent: ServiceStatePersistent::Enabled,
                },
            },
        );

        Ok(())
    })?;

    Ok(())
}

pub fn pkgstats<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.add(
        Ref("install pkgstats"),
        elevate,
        SinglePackage {
            package: Ref("pkgstats"),
            reason: InstallReason::Explicit,
            force: false,
        },
    );
    plan.register_packages(&[Ref("pkgstats")]);
}

pub fn iptables<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.block("iptables", |plan| {
        plan.add(
            Ref("install nftables"),
            elevate,
            SinglePackage {
                package: Ref("nftables"),
                reason: InstallReason::Explicit,
                force: false,
            },
        );

        plan.register_packages(&[Ref("nftables")]);

        plan.add(
            Ref("install iptables-legacy"),
            elevate,
            SinglePackage {
                package: Ref("iptables-legacy"),
                reason: InstallReason::Explicit,
                // conflicts with `iptables`, which is already present as
                // it's part of `base`
                force: true,
            },
        );

        plan.register_packages(&[Ref("iptables-legacy")]);

        plan.add(
            "configure iptables",
            elevate,
            File {
                path: Ref(Path::new("/etc/iptables/iptables.rules")),
                permissions: FilePermissions {
                    owner: Uid::root(),
                    group: Gid::root(),
                    mode: FileMode(0o644),
                },
                on_directory: DirectoryHandling::Bail,
                on_symlink: SymlinkHandling::Bail,
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(concat!(
                    dedent!(
                        r#"
                    *filter
                    :INPUT DROP [0:0]
                    :FORWARD DROP [0:0]
                    :OUTPUT ACCEPT [0:0]
                    -A INPUT -p icmp -j ACCEPT
                    -A INPUT -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT
                    -A INPUT -i lo -j ACCEPT
                    -A INPUT -i wg0 -p tcp --dport 22 -j ACCEPT
                    -A INPUT -p tcp -j REJECT --reject-with tcp-reset
                    -A INPUT -p udp -j REJECT --reject-with icmp-port-unreachable
                    -A INPUT -j REJECT --reject-with icmp-proto-unreachable
                    COMMIT
                "#
                    ),
                    "\n"
                )
                .as_bytes())),
            },
        )
        .on_change(|| {
            Resource::new(
                Ref("restart iptables"),
                elevate,
                RestartService {
                    name: "iptables.service",
                },
            )
        });

        plan.add(
            "enable iptables",
            elevate,
            Service {
                name: "iptables.service",
                state: ServiceState {
                    runtime: ServiceStateRuntime::Active,
                    persistent: ServiceStatePersistent::Enabled,
                },
            },
        );
    });
}

pub fn pacman<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.block("pacman", |plan| {
        plan.add(
            "configure pacman",
            elevate,
            File {
                path: Ref(Path::new("/etc/pacman.conf")),
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
                    r##"
                        [options]
                        Architecture = auto
                        HoldPkg = pacman glibc

                        # I have my own gpg agent user unit, don't want conflicts
                        NoExtract = usr/lib/systemd/user/gpg-agent*

                        # See https://bbs.archlinux.org/viewtopic.php?id=259764
                        NoExtract = usr/share/dbus-1/services/com.nextcloudgmbh.Nextcloud.service

                        # Managed by reflector
                        NoExtract = etc/pacman.d/mirrorlist

                        # Managed manually
                        NoExtract = etc/locale.gen

                        CheckSpace
                        ParallelDownloads = 5
                        DownloadUser = alpm

                        SigLevel = Required DatabaseOptional
                        LocalFileSigLevel = Optional

                        [core]
                        Include = /etc/pacman.d/mirrorlist

                        [extra]
                        Include = /etc/pacman.d/mirrorlist

                        [multilib]
                        Include = /etc/pacman.d/mirrorlist
                    "##
                )
                .as_bytes())),
            },
        )
        .on_change(|| {
            Resource::new(
                Ref("refresh package database"),
                elevate,
                PackageDatabaseRefresh {},
            )
        });

        plan.block("pacman-cache-cleanup", |plan| {
            let pacman_contrib = &[Ref("pacman-contrib")];
            plan.add(
                "install pacman-contrib",
                elevate,
                Packages {
                    install: Ref(pacman_contrib),
                    reason: InstallReason::Explicit,
                },
            );

            plan.register_packages(pacman_contrib);

            plan.add(
                "pacman cache cleanup service",
                elevate,
                File {
                    path: Ref(Path::new(
                        "/etc/systemd/system/pacman-cache-cleanup.service",
                    )),
                    on_directory: DirectoryHandling::Bail,
                    on_symlink: SymlinkHandling::Proceed,
                    permissions: FilePermissions {
                        owner: Uid::root(),
                        group: Gid::root(),
                        mode: FileMode(0o644),
                    },
                    binary: false,
                    secret: false,
                    ignore_checksum_changes: false,
                    content: FileContent::Lines(Ref(&[
                        Ref("[Service]"),
                        Ref("Type=oneshot"),
                        Ref("ExecStart=/bin/sh -c '/usr/bin/paccache -rk1 && /usr/bin/paccache -ruk0'"),
                        Ref("ExecStart=/bin/sh -c '/usr/bin/paccache -rk1 -c /var/lib/packages && /usr/bin/paccache -ruk0 -c /var/lib/packages'"),
                        Ref("RemainAfterExit=true"),
                    ])),
                },
            );

            plan.add(
                "pacman cache cleanup timer",
                elevate,
                File {
                    path: Ref(Path::new("/etc/systemd/system/pacman-cache-cleanup.timer")),
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
                    content: FileContent::Lines(Ref(&[
                        Ref("[Timer]"),
                        Ref("OnCalendar=daily"),
                        Ref(""),
                        Ref("[Install]"),
                        Ref("WantedBy=timers.target"),
                    ])),
                },
            );

            plan.add(
                "enable pacman cache cleanup timer",
                elevate,
                Service {
                    name: "pacman-cache-cleanup.timer",
                    state: ServiceState {
                        runtime: ServiceStateRuntime::Active,
                        persistent: ServiceStatePersistent::Enabled,
                    },
                },
            );
        });
    });
}

pub fn hardware<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
    config: &Config,
) {
    plan.block("hardware", |plan| {
        plan.block("gpu", |plan| match config.machine.gpu {
            Gpu::Amd => {
                let amdgpu_packages = &[
                    Ref("mesa"),
                    Ref("mesa-utils"),
                    Ref("lib32-mesa"),
                    Ref("vulkan-radeon"),
                    Ref("lib32-vulkan-radeon"),
                    Ref("vulkan-headers"),
                    Ref("vulkan-tools"),
                ];

                plan.add(
                    "install AMDGPU packages",
                    elevate,
                    Packages {
                        install: Ref(amdgpu_packages),
                        reason: InstallReason::Explicit,
                    },
                );

                plan.register_packages(amdgpu_packages);
            }
            Gpu::Nvidia => {
                let nouveau_packages = &[
                    Ref("mesa"),
                    Ref("mesa-utils"),
                    Ref("lib32-mesa"),
                    Ref("vulkan-nouveau"),
                    Ref("lib32-vulkan-nouveau"),
                    Ref("vulkan-headers"),
                    Ref("vulkan-tools"),
                ];

                plan.add(
                    "install nouveau packages",
                    elevate,
                    Packages {
                        install: Ref(nouveau_packages),
                        reason: InstallReason::Explicit,
                    },
                );

                plan.register_packages(nouveau_packages);
            }
            Gpu::Intel => {
                let intel_packages = &[
                    Ref("mesa"),
                    Ref("mesa-utils"),
                    Ref("lib32-mesa"),
                    Ref("vulkan-intel"),
                    Ref("lib32-vulkan-intel"),
                ];

                plan.add(
                    "install intel packages",
                    elevate,
                    Packages {
                        install: Ref(intel_packages),
                        reason: InstallReason::Explicit,
                    },
                );

                plan.register_packages(intel_packages);
            }
        });

        plan.block("cpu", |plan| match config.machine.cpu {
            Cpu::Amd => {
                let amd_cpu_packages = &[Ref("amd-ucode")];

                plan.add(
                    "install AMD cpu packages",
                    elevate,
                    Packages {
                        install: Ref(amd_cpu_packages),
                        reason: InstallReason::Explicit,
                    },
                );

                plan.register_packages(amd_cpu_packages);
            }
            Cpu::Intel => {
                let intel_cpu_packages = &[Ref("intel-ucode")];

                plan.add(
                    "install intel cpu packages",
                    elevate,
                    Packages {
                        install: Ref(intel_cpu_packages),
                        reason: InstallReason::Explicit,
                    },
                );

                plan.register_packages(intel_cpu_packages);
            }
        });
    });
}

pub fn backlight<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.add(
        // See https://wiki.archlinux.org/title/backlight#ACPI
        Ref("create udev rule to allow video group backlight access"),
        elevate,
        File {
            path: Ref(Path::new("/etc/udev/rules.d/backlight.rules")),
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
            content: FileContent::Lines(Ref(&[Ref(r#"ACTION=="add", SUBSYSTEM=="backlight", RUN+="/bin/chgrp video $sys$devpath/brightness", RUN+="/bin/chmod g+w $sys$devpath/brightness""#)])),
        },
    );
}

pub fn networkmanager<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.block("networkmanager", |plan| {
        let networkmanager = &[
            Ref("networkmanager"),
            Ref("networkmanager-dmenu"),
            Ref("nm-connection-editor"),
        ];
        plan.add(
            "install networkmanager",
            elevate,
            Packages {
                install: Ref(networkmanager),
                reason: InstallReason::Explicit,
            },
        );
        plan.register_packages(networkmanager);

        // https://wiki.archlinux.org/title/NetworkManager#Checking_connectivity
        plan.add(
            "disable connectivity checks",
            elevate,
            File {
                path: Ref(Path::new("/etc/NetworkManager/conf.d/20-connectivity.conf")),
                on_directory: DirectoryHandling::Bail,
                on_symlink: SymlinkHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::root(),
                    group: Gid::root(),
                    mode: FileMode(0o644),
                },
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                #[expect(
                    clippy::string_lit_as_bytes,
                    reason = "false positive due to the dedent!() macro"
                )]
                content: FileContent::Bytes(Ref(dedent!(
                    "
                    [connectivity]
                    enabled=false
                    "
                )
                .as_bytes())),
            },
        )
        .on_change(|| {
            Resource::new(
                Ref("restart networkmanager"),
                elevate,
                RestartService {
                    name: "NetworkManager.service",
                },
            )
        });

        plan.add(
            "enable networkmanager",
            elevate,
            Service {
                name: "NetworkManager.service",
                state: ServiceState {
                    runtime: ServiceStateRuntime::Active,
                    persistent: ServiceStatePersistent::Enabled,
                },
            },
        );
    });
}

pub fn filesystem<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    let lvm = &[Ref("lvm2")];
    plan.add(
        "install lvm",
        elevate,
        Packages {
            install: Ref(lvm),
            reason: InstallReason::Explicit,
        },
    );
    plan.register_packages(lvm);
}

pub fn timezone<'elevate: 'action, 'action, 'machine>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.block("timezone", |plan| {
        plan.add(
            "link timezone",
            elevate,
            Symlink {
                path: Ref(Path::new("/etc/localtime")),
                target: Ref(Path::new("/usr/share/zoneinfo/Europe/Berlin")),
                permissions: SymlinkPermissions {
                    owner: Uid::root(),
                    group: Gid::root(),
                },
                on_file: FileHandling::Proceed,
                on_directory: DirectoryHandling::Proceed,
            },
        );
    });
}

pub fn vconsole<'elevate: 'action, 'action, 'machine>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.add(
        "configure vconsole",
        elevate,
        File {
            path: Ref(Path::new("/etc/vconsole.conf")),
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
            content: FileContent::Bytes(Ref(concat!(
                dedent!(
                    r#"
                        KEYMAP=de-latin1
                        FONT=lat2-16
                    "#
                ),
                "\n"
            )
            .as_bytes())),
        },
    )
    .on_change(|| {
        Resource::new(
            Ref("restart vconsole config"),
            elevate,
            RestartService {
                name: "systemd-vconsole-setup.service",
            },
        )
    });
}

pub fn hostname<'elevate: 'action, 'action, 'machine>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
    config: &Config,
) {
    plan.add(
        "configure /etc/hostname",
        elevate,
        File {
            path: Ref(Path::new("/etc/hostname")),
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
            content: FileContent::Bytes(Ref(config.machine.host.as_str().as_bytes())),
        },
    );

    plan.add(
        "configure /etc/hosts",
        elevate,
        File {
            path: Ref(Path::new("/etc/hosts")),
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
            content: FileContent::Bytes(Owned(
                format!(
                    concat!(
                        dedent!(
                            r#"
                            127.0.0.1 localhost
                            ::1       localhost
                            127.0.1.1 {hostname}
                        "#
                        ),
                        "\n"
                    ),
                    hostname = config.machine.host.as_str()
                )
                .into_bytes(),
            )),
        },
    );
}

pub fn locale<'elevate: 'action, 'action, 'machine>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
    config: &Config,
) {
    plan.add(
        "configure locale.gen",
        elevate,
        File {
            path: Ref(Path::new("/etc/locale.gen")),
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
            content: FileContent::Lines(Owned(
                Language::VARIANTS
                    .iter()
                    .map(|lang| Owned(format!("{} UTF-8", lang.as_locale())))
                    .collect(),
            )),
        },
    )
    .on_change(|| {
        Resource::new(
            Ref("generate locale"),
            elevate,
            Command {
                cmd: Ref("locale-gen"),
                args: &[],
                check_command: None,
            },
        )
    });

    plan.add(
        "configure locale.conf",
        elevate,
        File {
            path: Ref(Path::new("/etc/locale.conf")),
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
            content: FileContent::Bytes(Owned(
                format!(
                    dedent!(
                        r#"
                    LANG={lang}
                "#
                    ),
                    lang = config.machine.language.as_locale()
                )
                .into_bytes(),
            )),
        },
    );
}

pub fn posix<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    let posix = &[Ref("posix")];
    plan.add(
        "install posix",
        elevate,
        Packages {
            install: Ref(posix),
            reason: InstallReason::Explicit,
        },
    );
    plan.register_packages(posix);
}

pub fn kernel<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.block("kernel", |plan| {
        let kernel = &[
            Ref("linux-zen"),
            Ref("linux-zen-headers"),
            Ref("linux-zen-docs"),
            Ref("linux-firmware"),
            Ref("man-pages"),
        ];
        plan.add(
            "install kernel",
            elevate,
            Packages {
                install: Ref(kernel),
                reason: InstallReason::Explicit,
            },
        );
        plan.register_packages(kernel);

        let bootloader = &[Ref("grub"), Ref("efibootmgr")];
        plan.add(
            "install bootloader",
            elevate,
            Packages {
                install: Ref(bootloader),
                reason: InstallReason::Explicit,
            },
        );
        plan.register_packages(bootloader);

        plan.add(
            "disable kernel modules",
            elevate,
            File {
                path: Ref(Path::new("/etc/modprobe.d/disable.conf")),
                permissions: FilePermissions {
                    owner: Uid::root(),
                    group: Gid::root(),
                    mode: FileMode(0o600),
                },
                on_directory: DirectoryHandling::Bail,
                on_symlink: SymlinkHandling::Proceed,
                binary: false,
                secret: false,
                ignore_checksum_changes: false,
                content: FileContent::Lines(Ref(&[
                    // dirty flag
                    Ref("install esp4 /bin/false"),
                    Ref("install esp6 /bin/false"),
                    Ref("install rxrpc /bin/false"),
                    Ref("blacklist esp4"),
                    Ref("blacklist esp6"),
                    Ref("blacklist rxrpc"),
                    // copyfail
                    Ref("install algif_aead /bin/false"),
                    Ref("install authencesn /bin/true"),
                    Ref("blacklist algif_aead"),
                    Ref("blacklist authencesn"),
                ])),
            },
        );
    });
}

pub fn journald<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.block("journald", |block| {
        block
            .add(
                "configure journald",
                elevate,
                File {
                    path: Ref(Path::new("/etc/systemd/journald.conf")),
                    permissions: FilePermissions {
                        owner: Uid::root(),
                        group: Gid::root(),
                        mode: FileMode(0o600),
                    },
                    on_symlink: SymlinkHandling::Bail,
                    on_directory: DirectoryHandling::Bail,
                    secret: false,
                    binary: false,
                    ignore_checksum_changes: false,
                    content: FileContent::Bytes(Ref(concat!(
                        dedent!(
                            r#"
                                [Journal]
                                SystemMaxUse=50M
                            "#
                        ),
                        "\n"
                    )
                    .as_bytes())),
                },
            )
            .on_change(|| {
                Resource::new(
                    Ref("restart journald"),
                    elevate,
                    RestartService {
                        name: "systemd-journald.service",
                    },
                )
            });
    });
}

pub fn logind<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.add(
        "configure logind.conf",
        elevate,
        File {
            path: Ref(Path::new("/etc/systemd/logind.conf")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o600),
            },
            on_directory: DirectoryHandling::Bail,
            on_symlink: SymlinkHandling::Proceed,
            binary: false,
            secret: false,
            ignore_checksum_changes: false,
            content: FileContent::Lines(Ref(&[
                Ref("[Login]"),
                Ref("HandlePowerKey=suspend"),
                Ref("HandleLidSwitch=ignore"),
            ])),
        },
    );
}

pub fn firefox<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.block("firefox", |plan| {
        for directory in [
            Path::new("/etc/firefox"),
            Path::new("/etc/firefox/policies"),
        ] {
            plan.add(
                Owned(format!("create firefox directories {directory}")),
                elevate,
                Directory {
                    path: Ref(directory),
                    on_symlink: SymlinkHandling::Proceed,
                    on_file: FileHandling::Proceed,
                    permissions: FilePermissions {
                        owner: Uid::root(),
                        group: Gid::root(),
                        mode: FileMode(0o775),
                    },
                },
            );
        }

        #[expect(clippy::default_numeric_fallback, reason = "")]
        let policies = serde_json::json!({
            "policies": {
                "AutofillAddressEnabled": false,
                "AutofillCreditCardEnabled": false,
                "DownloadDirectory": "${home}/download",
                "DisableFeedbackCommands": true,
                "DisableFirefoxAccounts": true,
                "DisableFirefoxStudies": true,
                "DisableForgetButton": true,
                "DisableMasterPasswordCreation": true,
                "DisableProfileImport": true,
                "DisableProfileRefresh": true,
                "DisableSafeMode": true,
                "DisableSetDesktopBackground": true,
                "DisableTelemetry": true,
                "DisplayBookmarksToolbar": "always",
                "DisplayMenuBar": "default-off",
                "DontCheckDefaultBrowser": true,
                "EnableTrackingProtection": {
                    "Value": true,
                    "Locked": false,
                    "Category": "strict",
                    "BaselineExceptions": true,
                    "ConvenienceExceptions": false
                },
                "ExtensionSettings": {
                    "*": {
                        "allowed_types": [
                            "extension"
                        ]
                    },
                    "jid1-KKzOGWgsW3Ao4Q@jetpack": {
                        "installation_mode": "normal_installed",
                        "install_url": "https://addons.mozilla.org/firefox/downloads/file/4202634/i_dont_care_about_cookies.xpi",
                        "default_area": "menupanel",
                        "private_browsing": true,
                        "updates_disabled": false
                    },
                    "uBlock0@raymondhill.net": {
                        "installation_mode": "normal_installed",
                        "install_url": "https://addons.mozilla.org/firefox/downloads/file/4598854/ublock_origin-1.67.0.xpi",
                        "default_area": "navbar",
                        "private_browsing": true,
                        "updates_disabled": false
                    },
                    "treestyletab@piro.sakura.ne.jp": {
                        "installation_mode": "normal_installed",
                        "install_url": "https://addons.mozilla.org/firefox/downloads/file/4602712/tree_style_tab-4.2.7.xpi",
                        "default_area": "navbar",
                        "private_browsing": true,
                        "updates_disabled": false
                    },
                    "{9063c2e9-e07c-4c2c-9646-cfe7ca8d0498}": {
                        "installation_mode": "normal_installed",
                        "install_url": "https://addons.mozilla.org/firefox/downloads/file/4526031/old_reddit_redirect-2.0.9.xpi",
                        "default_area": "menupanel",
                        "private_browsing": true,
                        "updates_disabled": false
                    }
                },
                "FirefoxHome": {
                    "Search": false,
                    "TopSites": false,
                    "SponsoredTopSites": false,
                    "Highlights": false,
                    "Pocket": false,
                    "Stories": false,
                    "SponsoredPocket": false,
                    "SponsoredStories": false,
                    "Snippets": false,
                    "Locked": true
                },
                "GenerativeAI": {
                    "Enabled": false,
                    "Chatbot": false,
                    "LinkPreviews": false,
                    "TabGroups": false
                },
                "Homepage": {
                    "URL": "about:newtab",
                    "StartPage": "previous-session"
                },
                "MicrosoftEntraSSO": false,
                "NewTabPage": false,
                "NoDefaultBookmarks": true,
                "OfferToSaveLogins": false,
                "OverrideFirstRunPage": "",
                "PasswordManagerEnabled": false,
                "Preferences": {
                    "browser.translations.automaticallyPopup": {
                        "Value": false,
                        "Status": "default",
                        "Type": "boolean"
                    },
                    "browser.aboutConfig.showWarning": {
                        "Value": false,
                        "Status": "default",
                        "Type": "boolean"
                    },
                    "general.smoothScroll": {
                        "Value": true,
                        "Status": "default",
                        "Type": "boolean"
                    },
                    "media.eme.enabled": {
                        "Value": true,
                        "Status": "default",
                        "Type": "boolean"
                    },
                    "browser.startup.page": {
                        "Value": 3,
                        "Status": "default",
                        "Type": "number"
                    },
                    "browser.sessionstore.restore_on_demand": {
                        "Value": false,
                        "Status": "default",
                        "Type": "boolean"
                    },
                    "layout.spellcheckDefault": {
                        "Value": 0,
                        "Status": "default",
                        "Type": "number"
                    },
                    "dom.private-attribution.submission.enabled": {
                        "Value": false,
                        "Status": "default",
                        "Type": "boolean"
                    },
                    "app.update.auto": {
                        "Value": false,
                        "Status": "default",
                        "Type": "boolean"
                    },
                    "app.update.service.enabled": {
                        "Value": false,
                        "Status": "default",
                        "Type": "boolean"
                    },
                    // Otherwise, userChrome.css does not apply
                    "toolkit.legacyUserProfileCustomizations.stylesheets": {
                        "Value": true,
                        "Status": "default",
                        "Type": "boolean"
                    },
                },
                "PromptForDownloadLocation": false,
                "RequestedLocales": [
                    "en-US",
                    "de"
                ],
                "SearchSuggestEnabled": false,
                "ShowHomeButton": false,
                "SkipTermsOfUse": true,
                "UserMessaging": {
                    "ExtensionRecommendations": false,
                    "FeatureRecommendations": false,
                    "UrlbarInterventions": false,
                    "SkipOnboarding": true,
                    "MoreFromMozilla": false,
                    "FirefoxLabs": false
                },
                "VisualSearchEnabled": false
            }
        });

        plan.add(
            Ref("set firefox global policies"),
            elevate,
            File {
                path: Ref(Path::new("/etc/firefox/policies/policies.json")),
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
                content: FileContent::Bytes(Owned(
                    serde_json::to_vec_pretty(&policies)
                        .expect("policies are builtin, will always be serializable"),
                )),
            },
        );
    });
}

async fn unmanaged_packages<'elevate: 'action, 'action, 'pkgs>(
    managed_packages: &[MaybeOwned<'pkgs, str>],
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) -> Result<()> {
    let protected = [
        "base", // virtual package, would not hurt, but it's weird if its not installed
    ];

    let managed_packages = managed_packages
        .iter()
        .map(|s| s.to_owned())
        .collect::<HashSet<String>>();

    let explicitly_installed_packages = list_explicitly_installed_packages(elevate).await?;

    #[expect(clippy::iter_over_hash_type, reason = "order does not matter")]
    for installed_package in explicitly_installed_packages {
        if !managed_packages.contains(&installed_package)
            && !protected.contains(&installed_package.as_str())
        {
            plan.add(
                Owned(format!("remove unmanaged package {installed_package}")),
                elevate,
                SinglePackage {
                    package: Owned(installed_package),
                    reason: InstallReason::Dependency,
                    force: false,
                },
            );
        }
    }

    Ok(())
}

async fn orphans<'name, 'elevate: 'action, 'pkgs, 'action, 'secrets>(
    plan: &mut Plan<'name, 'elevate, 'pkgs, 'action, 'secrets, Secrets>,
    elevate: &'elevate Elevate,
) -> Result<()> {
    let orphans = list_orphaned_packages(elevate).await?;

    #[expect(clippy::iter_over_hash_type, reason = "order does not matter")]
    for orphan in orphans {
        plan.add(
            Owned(format!("remove orphaned package {orphan}")),
            elevate,
            PackageRemove {
                remove: Owned(vec![Owned(orphan)]),
            },
        );
    }

    Ok(())
}

pub async fn remove_unconfigured_packages(
    root: &Elevate,
    dry_run: bool,
    managed_packages: &[MaybeOwned<'_, str>],
) -> Result<()> {
    // Removed packages may produce orphans, so we apply this until there
    // is nothing more to remove
    //
    // We also do not remove unmanaged packages immediately, but just mark
    // them as "installed as dependency". It can happen that a package is
    // installed explicitly while *also* being a dependency of another package.
    // We cannot rip it out immedaitely, but mark is as dependency and then
    // wait for orphan removal to get rid of it.
    loop {
        let mut unmanaged_package_plan = Plan::new();

        unmanaged_packages(managed_packages, &mut unmanaged_package_plan, root).await?;

        let cleanup_plan_finished = match unmanaged_package_plan.finish().await? {
            FinishResult::AssertionsFailed { report: _ } => {
                unreachable!("removal plan does not contain assertions")
            }
            FinishResult::Success(plan) => plan,
        };

        let cleanup_report = cleanup_plan_finished.report()?;

        let has_changes = cleanup_report.has_changes();

        let cleanup_plan_approved = cleanup_plan_finished.approve();

        if !dry_run && has_changes {
            cleanup_plan_approved.apply().await?;
        } else {
            break;
        }
    }

    loop {
        let mut orphaned_package_plan = Plan::new();

        orphans(&mut orphaned_package_plan, root).await?;

        let cleanup_plan_finished = match orphaned_package_plan.finish().await? {
            FinishResult::AssertionsFailed { report: _ } => {
                unreachable!("removal plan does not contain assertions")
            }
            FinishResult::Success(plan) => plan,
        };

        let cleanup_report = cleanup_plan_finished.report()?;

        let has_changes = cleanup_report.has_changes();

        let cleanup_plan_approved = cleanup_plan_finished.approve();

        if !dry_run && has_changes {
            cleanup_plan_approved.apply().await?;
        } else {
            break;
        }
    }

    Ok(())
}
