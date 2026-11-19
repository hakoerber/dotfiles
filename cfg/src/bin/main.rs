#![expect(
    clippy::large_include_file,
    reason = "Everything necessary should be included in the binary (PKGBUILDS, locally built packages)"
)]
#![deny(unused_must_use)]
// required for `serde_json::json!()`
#![recursion_limit = "256"]

use std::{
    collections::{HashMap, HashSet},
    env, io,
    net::Ipv4Addr,
    process,
};

use anyhow::{Result, anyhow, bail};
use clap::{Parser, Subcommand, ValueEnum};
use dedent::dedent;
use include_dir::{DirEntry, include_dir};
use serde::{Deserialize, Serialize};
use strum::VariantArray;

use cfg::prelude::*;

static BINS: include_dir::Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../files/bin");

static SCRIPTS: include_dir::Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../files/scripts");

static INCLUDED_PACKAGES: &[(&str, &str, &str, &[u8])] =
    include!(concat!(env!("OUT_DIR"), "/packages.rs"));

#[expect(clippy::print_stdout, reason = "migrations are chatty")]
mod migrations {
    use std::fmt;

    use anyhow::Result;
    use serde::{Deserialize, Serialize};

    use cfg::prelude::*;

    const STATE_PATH: &str = "/var/lib/cfg.state";

    #[derive(Serialize, Deserialize, Debug)]
    pub(super) struct MachineState {
        migration: Migration,
    }

    const LATEST_MIGRATION: usize = 4;

    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub(super) struct Migration {
        id: usize,
    }

    struct MigrationIter {
        migration: Migration,
    }

    impl Iterator for MigrationIter {
        type Item = Migration;

        fn next(&mut self) -> Option<Self::Item> {
            if self.migration.id >= LATEST_MIGRATION {
                None
            } else {
                self.migration.id = self
                    .migration
                    .id
                    .checked_add(1)
                    .expect("bounded by LATEST_MIGRATION");
                Some(self.migration.clone())
            }
        }
    }

    impl Migration {
        pub(super) fn apply(&self, elevate: &Elevate) -> Result<()> {
            #[expect(
                clippy::match_overlapping_arm,
                reason = "make sure that bumping LATEST_MIGRATION forces adding it here"
            )]
            match self.id {
                0 => unreachable!("initial migration will never be applied"),
                1 => {
                    Process::run_in_foreground(
                        "bash",
                        &[
                            "-c",
                            "
                            yes | pacman --needed -S pipewire pipewire-pulse
                        ",
                        ],
                        elevate,
                        &Stdin::Closed,
                    )?
                    .check()?;
                }
                2 => {
                    Process::run_in_foreground(
                        "bash",
                        &[
                            "-c",
                            "
                            set -o nounset
                            set -o errexit
                            set -o pipefail

                            rm -f /etc/conf.d/libvirtd
                            systemctl disable libvirtd.service
                            systemctl stop libvirtd.service
                        ",
                        ],
                        elevate,
                        &Stdin::Closed,
                    )?
                    .check()?;
                }
                3 => {
                    Process::run_in_foreground(
                        "bash",
                        &[
                            "-c",
                            r#"
                            set -o nounset
                            set -o errexit
                            set -o pipefail

                            cryptdev="$(lsblk --filter 'FSTYPE == "crypto_LUKS"' --output NAME --noheadings)"

                            if [[ -n "${cryptdev}" ]] ; then
                                if ! cryptsetup luksDump "/dev/${cryptdev}" | grep ^Flags | grep -q allow-discards ; then
                                    cryptmapper="$(cat /proc/cmdline | grep -o 'cryptdevice=[^ ]*' | cut -d ':' -f 2)"
                                    printf 'you need to enable discards in luks via the following command:\n' 2>&1
                                    printf 'cryptsetup --allow-discards --persistent refresh %s\n' "${cryptmapper}" 2>&1
                                    exit 1
                                fi
                            fi
                        "#,
                        ],
                        elevate,
                        &Stdin::Closed,
                    )?
                    .check()?;
                }
                LATEST_MIGRATION => {
                    Process::run_in_foreground(
                        "bash",
                        &[
                            "-c",
                            r#"
                            set -o nounset
                            set -o errexit
                            set -o pipefail

                            lsblk --filter 'FSTYPE == "ext4"' --output PATH --noheadings | while read device ; do
                                if ! tune2fs -l "${device}" | grep '^Default mount options:' | grep -q discard ; then
                                    tune2fs -o discard "${device}" >/dev/null
                                fi
                            done
                        "#,
                        ],
                        elevate,
                        &Stdin::Closed,
                    )?
                    .check()?;
                }
                LATEST_MIGRATION.. => unreachable!(),
            }

            Ok(())
        }

        fn unapplied(&self) -> MigrationIter {
            MigrationIter {
                migration: self.clone(),
            }
        }

        pub(crate) fn latest() -> Self {
            Self {
                id: LATEST_MIGRATION,
            }
        }
    }

    impl fmt::Display for Migration {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "(id: {})", self.id)
        }
    }

    impl MachineState {
        pub(super) async fn load(elevate: &Elevate) -> Result<Option<Self>> {
            Ok(try_read(elevate, Path::new(STATE_PATH))
                .await?
                .map(|file| serde_json::from_slice(&file))
                .transpose()?)
        }

        pub(super) async fn save(&self, elevate: &Elevate) -> Result<()> {
            let data = serde_json::to_string(self)?;
            create_or_write(Path::new(STATE_PATH), data.as_bytes(), elevate).await?;
            Ok(())
        }

        pub(super) async fn apply_migrations(
            &mut self,
            elevate: &Elevate,
            dry_run: bool,
        ) -> Result<bool> {
            let mut any_applied = false;
            for migration in self.migration.unapplied() {
                if dry_run {
                    println!("would apply {migration}");
                } else {
                    println!("applying {migration}");
                    migration.apply(elevate)?;
                    self.migration = migration;
                    self.save(elevate).await?;
                    any_applied = true;
                }
            }
            Ok(any_applied)
        }

        pub(super) fn new(migration: Migration) -> Self {
            Self { migration }
        }
    }

    impl fmt::Display for MachineState {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "State({})", self.migration)
        }
    }
}

const MY_SSH_PUBKEY: &str = "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAACAQCxOtVxz3vRL7SUZVO899RUcg9BiDuZfPUfsd9v3AHOwNvSVsLObqq0QgwajmTML9nRZzX7kc7RmFtXZ5ypMkyEJ9p14rYXKYKhLqEM4Paz0lUagQhy0sdYijtayNsM5luU3yk6a4F/PkanprDGEEtJPMLCwpDODTwlE7THvFtJXLnYeFtF2DJKnUcP6E9MWGYM1zYdrmf43pxWg2YXFjoyLEkOy6TVjjqwz+tKa6+4YRqFWNyXcEWqI2ZLqHRzZTvu+46gsnR+8HcLpov3ij/Mr0jYSJ4vqMGz1yfKnrsKZUp0IhvlPYbGjdcL17xYame2UroPS8AzWWegyqeEFh54qmCrFVw/uwErcWcUJX3KEijrc48DYZUBK09vnFFcXIYR/cfFWpTGhnqlwp/EAaBeR3zr6hS6lL+XluMIjHHkwT/eD3YydEwABjbJwBJp5PdYR4M/bSHuOoCH6d/U36OpkwwekyYKWvEoxxPSJIWCy1ApILQZmWuctIFc8SQfSXfRifPEDBnL9hNzzX00beDYmS6cPIP0TIqa9fdL1AeO68Tk7o67f2cCE11QTH7FI9JjhuQ/M4dO8jUw7kmtGCKdReNIuYYU3xZB4wdFHs/5UxROCwEZnv+3uWAgHbgc14XLWunN9P9msOxYLHPVJK/XcdhHvWSmQdNq4ajry/H+kQ==";

type PackageList<'pkgs> =
    HashMap<MaybeOwned<'pkgs, str>, MaybeOwned<'pkgs, [MaybeOwned<'pkgs, str>]>>;

#[derive(Serialize)]
enum Cpu {
    Intel,
    Amd,
}

#[derive(Serialize)]
enum Gpu {
    Amd,
    Nvidia,
    Intel,
}

#[derive(Clone, Copy, ValueEnum)]
enum Host {
    Neptune,
    Ares,
    Dionysus,
    Hera,
}

impl Host {
    fn as_str(self) -> &'static str {
        match self {
            Neptune => "neptune",
            Ares => "ares",
            Dionysus => "dionysus",
            Hera => "hera",
        }
    }
}

use Host::{Ares, Dionysus, Hera, Neptune};

#[derive(Deserialize, Clone)]
struct Secret<T: Clone>(T);

impl<T: Clone> std::fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("[redacted]").finish()
    }
}

impl<T: Clone> Secret<T> {
    fn access(&self) -> &T {
        &self.0
    }
}

#[derive(Deserialize, Clone)]
struct WireguardPubkey(String);

impl WireguardPubkey {
    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize, Clone)]
struct WireguardPrivkey(String);

impl WireguardPrivkey {
    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize, Clone)]
struct WireguardPsk(String);

impl WireguardPsk {
    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize, Clone)]
struct Domain(String);

impl Domain {
    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize, Clone)]
struct Port(u16);

impl Port {
    fn as_inner(&self) -> u16 {
        self.0
    }
}

#[derive(Debug, Deserialize, Clone)]
struct WireguardSecrets {
    address: Secret<Ipv4Addr>,
    privkey: Secret<WireguardPrivkey>,
    psk: Secret<WireguardPsk>,
    server_pubkey: Secret<WireguardPubkey>,
    server_ip: Secret<Ipv4Addr>,
    server_endpoint_domain: Secret<Domain>,
    server_endpoint_port: Secret<Port>,
}

#[derive(Debug, Deserialize, Clone)]
struct Secrets {
    wireguard: WireguardSecrets,
}

impl Secrets {
    fn load_from(reader: impl io::Read) -> Result<Self> {
        Ok(serde_json::from_reader(reader)?)
    }
}

#[derive(Subcommand)]
enum Action {
    Deploy {
        #[arg(long)]
        dry_run: bool,

        #[arg(long)]
        only_changed: bool,

        #[arg(long)]
        clean_packages: bool,

        #[arg(long)]
        secrets_stdin: bool,
    },
    Bootstrap {
        #[arg(long)]
        i_am_sure: bool,
    },
    Migrate {
        #[arg(long)]
        dry_run: bool,

        #[arg(long)]
        init: bool,
    },
}

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    debug: bool,

    #[arg(long)]
    hostname: Host,

    #[command(subcommand)]
    action: Action,
}

const APPLICATION_PACKAGES: &[(&str, &[&str])] = if cfg!(feature = "testing") {
    &[]
} else {
    &[
        ("curl", &["curl"]),
        ("wget", &["wget"]),
        ("pwgen", &["pwgen"]),
        (
            "gpg",
            &[
                "gnupg",
                "paperkey",
                "yubikey-manager",
                "yubikey-touch-detector",
            ],
        ),
        ("dunst", &["dunst"]),
        ("rofi", &["rofi"]),
        ("imv", &["imv"]),
        ("jq", &["jq"]),
        ("ripgrep", &["ripgrep"]),
        ("fzf", &["fzf"]),
        (
            "build-essentials",
            &["gcc", "ctags", "cmake", "maven", "base-devel"],
        ),
        ("strace", &["strace"]),
        (
            "arch-packaging",
            &["namcap", "devtools", "arch-install-scripts", "archiso"],
        ),
        ("lsp/systemd", &["systemd-lsp"]),
        ("qemu", &["qemu-base", "qemu-desktop"]),
        ("make", &["make"]),
        ("gdb", &["gdb"]),
        ("font-utils", &["woff2"]),
        ("restic", &["restic"]),
        ("neovim", &["neovim", "python-pynvim"]),
        ("nodejs", &["nodejs", "npm", "yarn"]),
        ("docker", &["dockerfile-language-server"]),
        ("ansible", &["ansible", "ansible-lint", "python-jmespath"]),
        ("htop", &["htop"]),
        ("which", &["which"]),
        ("zip", &["zip", "unzip"]),
        ("tar", &["tar"]),
        ("sshfs", &["sshfs"]),
        ("expect", &["expect"]),
        ("inotify", &["inotify-tools"]),
        ("bwm-ng", &["bwm-ng"]),
        ("pv", &["pv"]),
        ("lxappearance", &["lxappearance"]),
        ("stress", &["stress"]),
        ("ncdu", &["ncdu"]),
        (
            "python",
            &[
                "python",
                "python-lsp-server",
                "bandit",
                "python-pyflakes",
                "python-pylint",
                "python-flake8",
                "mypy",
                "uv",
                "ty",
                "jedi-language-server",
            ],
        ),
        ("dust", &["dust"]),
        ("rclone", &["rclone"]),
        ("wild", &["wild"]),
        ("rsync", &["rsync"]),
        ("net-tools", &["net-tools"]),
        ("ntfs", &["ntfs-3g"]),
        ("lsof", &["lsof"]),
        ("iw", &["iw"]),
        ("awscli", &["aws-cli-v2"]),
        ("php", &["php", "composer"]),
        ("cowsay", &["cowsay"]),
        ("ruby", &["ruby"]),
        ("zathura", &["zathura", "zathura-pdf-poppler"]),
        ("pdf", &["ghostscript", "enscript"]),
        ("ranger", &["ranger"]),
        ("thunar", &["thunar"]),
        (
            "libreoffice",
            &["libreoffice-fresh", "libreoffice-fresh-de"],
        ),
        ("bind", &["bind"]),
        ("nmap", &["nmap"]),
        ("tree", &["tree"]),
        ("traceroute", &["traceroute"]),
        ("tcpdump", &["tcpdump"]),
        ("musescore", &["musescore"]),
        ("imagemagick", &["imagemagick"]),
        ("mpv", &["mpv"]),
        ("terraform", &["terraform", "terragrunt", "tflint"]),
        ("kubectl", &["kubectl"]),
        ("chromium", &["chromium"]),
        ("go", &["go", "gopls", "delve", "golangci-lint"]),
        ("mariadb-client", &["mariadb-clients"]),
        ("mdbook", &["mdbook"]),
        ("packer", &["packer"]),
        ("johntheripper", &["john"]),
        ("just", &["just", "just-lsp"]),
        ("fd", &["fd"]),
        ("bat", &["bat"]),
        (
            "bash",
            &["bash", "bash-language-server", "shellcheck", "shfmt"],
        ),
        ("age", &["age"]),
        ("httpie", &["httpie"]),
        ("c", &["gcc", "clang"]),
        ("html", &["vscode-html-languageserver"]),
        ("typescript", &["typescript", "typescript-language-server"]),
        ("json", &["vscode-json-languageserver", "gron"]),
        ("lldb", &["lldb"]),
        ("yaml", &["yaml-language-server", "yamllint"]),
        ("cloud-init", &["cloud-init", "cloud-utils"]),
        ("netcat", &["openbsd-netcat"]),
        ("telnet", &["inetutils"]),
        ("cloudformation-tools", &["python-cfn-lint"]),
        ("kcharselect", &["kcharselect"]),
        ("sysstat", &["sysstat"]),
        ("qrencode", &["qrencode"]),
        ("iotop", &["iotop"]),
        ("w3m", &["w3m"]),
        ("xpath", &["perl-xml-xpath"]),
        ("apache", &["apache"]),
        ("xfce4-screenshooter", &["xfce4-screenshooter"]),
        ("fuseiso", &["fuseiso"]),
        ("7zip", &["7zip"]),
        ("unrar", &["unrar"]),
        ("wasm", &["wasm-pack"]),
        ("css", &["vscode-css-languageserver"]),
        ("whois", &["whois"]),
        ("encfs", &["encfs"]),
        ("iftop", &["iftop"]),
        (
            "wine",
            &[
                "wine",
                "wine-gecko",
                "wine-mono",
                "winetricks",
                "lib32-libpulse",
                "vkd3d",
                "lib32-vkd3d",
                "lib32-gnutls",
            ],
        ),
        ("yt-dlp", &["yt-dlp"]),
        ("ytfzf", &["ytfzf"]),
        ("ffmpeg", &["ffmpeg"]),
        ("ruff", &["ruff"]),
        ("sqlite", &["sqlite"]),
        ("watchexec", &["watchexec"]),
        ("postgresql", &["postgresql", "pgformatter"]),
        ("tokei", &["tokei"]),
        ("digikam", &["digikam"]),
        ("evince", &["evince"]),
        ("inkscape", &["inkscape"]),
        ("android", &["android-tools", "android-udev"]),
        ("audacity", &["audacity"]),
        ("calibre", &["calibre"]),
        ("deluge", &["deluge", "deluge-gtk"]),
        ("sqlx", &["sqlx-cli"]),
        ("geeqie", &["geeqie"]),
        ("toml", &["taplo-cli", "tombi"]),
        ("prettier", &["prettier"]),
        ("btop", &["btop"]),
        ("dua", &["dua-cli"]),
        ("hexyl", &["hexyl"]),
        ("yq", &["go-yq"]),
        ("kolourpaint", &["kolourpaint"]),
        ("podman", &["podman", "podman-compose"]),
        ("pulumi", &["pulumi"]),
        (
            "yazi",
            &[
                "yazi",
                "ffmpeg",
                "7zip",
                "jq",
                "poppler",
                "fd",
                "ripgrep",
                "fzf",
                "xsel",
                "zoxide",
                "resvg",
                "imagemagick",
            ],
        ),
        ("filezilla", &["filezilla"]),
        ("protontricks", &["protontricks"]),
        ("glab", &["glab"]),
        ("mousepad", &["mousepad"]),
        ("graphviz", &["graphviz", "xdot"]),
        ("markdown", &["marksman", "mdformat"]),
        ("magic-wormhole", &["magic-wormhole"]),
    ]
};

fn autoupdate<'name: 'static, 'elevate: 'action, 'pkgs, 'action, 'secrets>(
    plan: &mut Block<'name, 'elevate, 'pkgs, 'action, 'secrets, Secrets>,
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

fn fstrim<'elevate: 'action, 'action, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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

fn restic_backup<'elevate: 'action, 'action>(
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

fn initrd<'name, 'elevate: 'action, 'action>(
    plan: &mut Plan<'name, 'elevate, '_, 'action, '_, Secrets>,
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

fn libvirt<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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

fn docker<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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

fn wireshark<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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

fn smartcard<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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

fn aur<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.block("aur", |plan| {
        let included_packages_directory = Path::new("/var/lib/packages");

        plan.add(
            Ref("create included packages directory"),
            elevate,
            Directory {
                path: Owned(included_packages_directory.to_owned()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::root(),
                    group: Gid::root(),
                    mode: FileMode(0o700),
                },
            },
        );

        for &(name, version, filename, content) in INCLUDED_PACKAGES {
            plan.block(name, |plan| {
                let package_path = included_packages_directory.join(filename);

                plan.add(
                    Ref("deploy included package"),
                    elevate,
                    File {
                        content: FileContent::Bytes(Ref(content)),
                        binary: true,
                        // checksum may change, the only thing that matters is the version, which is
                        // part of the filename
                        ignore_checksum_changes: true,
                        secret: false,
                        path: Owned(package_path.clone()),
                        on_symlink: SymlinkHandling::Proceed,
                        on_directory: DirectoryHandling::Bail,
                        permissions: FilePermissions {
                            owner: Uid::root(),
                            group: Gid::root(),
                            mode: FileMode(0o600),
                        },
                    },
                );

                plan.add(
                    Ref("install included package"),
                    elevate,
                    LocalPackage {
                        name: Ref(name),
                        path: Owned(package_path),
                        version: Ref(version),
                        reason: InstallReason::Explicit,
                    },
                );

                plan.register_packages(&[Ref(name)]);
            });
        }
    });
}

fn boot_target<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
    config: &Config,
) {
    plan.add(
        Ref("setting boot target"),
        elevate,
        BootTarget {
            target: config.machine.boot_target,
        },
    );
}

fn sudo<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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
                content: Lines(Owned(vec![Ref("%sudonopw ALL=(ALL) NOPASSWD: ALL")])),
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
                content: Lines(Owned(vec![Ref("permit nopass nolog setenv {PATH=/usr/local/bin:/usr/local/sbin:/usr/bin:/usr/sbin} :sudonopw")])),
            },
        );
    });
}

fn timesyncd<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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
                content: Lines(Owned(vec![
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

fn reflector<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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
                content: Lines(Ref(&[
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

fn wireguard<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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

fn pipewire<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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

fn fonts<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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

fn sshd<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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

fn pkgstats<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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

fn iptables<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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

fn pacman<'plan, 'elevate: 'action, 'action>(
    plan: &'plan mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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
                    content: Lines(Ref(&[
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
                    content: Lines(Ref(&[
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

fn hardware<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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

fn backlight<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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
            content: Lines(Ref(&[Ref(r#"ACTION=="add", SUBSYSTEM=="backlight", RUN+="/bin/chgrp video $sys$devpath/brightness", RUN+="/bin/chmod g+w $sys$devpath/brightness""#)])),
        },
    );
}

fn networkmanager<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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

fn filesystem<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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

fn timezone<'elevate: 'action, 'action, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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

fn vconsole<'elevate: 'action, 'action, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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

fn hostname<'elevate: 'action, 'action, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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
            content: FileContent::Bytes(Ref(config.machine.hostname.as_bytes())),
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
                            127.0.0.1 {hostname}
                        "#
                        ),
                        "\n"
                    ),
                    hostname = config.machine.hostname
                )
                .into_bytes(),
            )),
        },
    );
}

fn locale<'elevate: 'action, 'action, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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

fn posix<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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

fn kernel<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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
                content: Lines(Ref(&[
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

fn journald<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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

fn logind<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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
            content: Lines(Ref(&[
                Ref("[Login]"),
                Ref("HandlePowerKey=suspend"),
                Ref("HandleLidSwitch=ignore"),
            ])),
        },
    );
}

fn firefox<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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
                "DefaultDownloadDirectory": "${home}/download",
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

struct UserDirectories {
    config: PathBuf,
    bin: PathBuf,
    scripts: PathBuf,
    tmp: PathBuf,
    download: PathBuf,
    systemd: PathBuf,
    systemd_user_units: PathBuf,
    local: PathBuf,
    data: PathBuf,
    state: PathBuf,
    applications: PathBuf,
}

fn root_setup<'elevate: 'action, 'action, 'pkgs>(
    plan: &mut Plan<'_, 'elevate, 'pkgs, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
    config: &Config,
) -> Result<()> {
    kernel(plan, elevate);
    filesystem(plan, elevate);
    hostname(plan, elevate, config);
    locale(plan, elevate, config);
    vconsole(plan, elevate);
    fstrim(plan, elevate);
    timezone(plan, elevate);
    pacman(plan, elevate);
    posix(plan, elevate);
    networkmanager(plan, elevate);
    reflector(plan, elevate);
    initrd(plan, elevate, config);
    libvirt(plan, elevate);
    docker(plan, elevate);
    pipewire(plan, elevate);
    fonts(plan, elevate);
    smartcard(plan, elevate);
    wireshark(plan, elevate);
    boot_target(plan, elevate, config);
    timesyncd(plan, elevate);
    sudo(plan, elevate);
    hardware(plan, elevate, config);
    logind(plan, elevate);
    journald(plan, elevate);
    wireguard(plan, elevate)?;
    sshd(plan, elevate)?;
    pkgstats(plan, elevate);
    iptables(plan, elevate);
    backlight(plan, elevate);
    firefox(plan, elevate);

    Ok(())
}

struct UserResult<'pkgs> {
    packages: HashMap<MaybeOwned<'pkgs, str>, MaybeOwned<'pkgs, [MaybeOwned<'pkgs, str>]>>,
}

async fn configure_user<'elevate: 'action, 'action, 'user: 'action, 'machine, 'pkgs>(
    plan: &mut Block<'_, 'elevate, 'pkgs, 'action, '_, Secrets>,
    root: &'elevate Elevate,
    user: &'elevate UserContext,
    machine: &'machine MachineConfig,
) -> Result<UserResult<'pkgs>> {
    let mut packages = HashMap::new();

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
        user_autologin(plan, root, user);
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

    user_create_directories(plan, user_elevate, user, &user_directories);

    let mut required_packages =
        user_dotfiles(plan, user_elevate, user, machine, &home, &user_directories)?;

    let dependencies = Box::pin(user_executables(
        plan,
        user_elevate,
        user,
        &user_directories,
    ))
    .await?;

    required_packages.extend(dependencies);

    packages.extend(required_packages);

    packages.insert(
        Ref("pass"),
        Ref([Ref("pass"), Ref("passff-host")].as_slice()),
    );

    packages.insert(
        Ref("theming"),
        Ref([Ref("breeze"), Ref("breeze-grub"), Ref("breeze-gtk")].as_slice()),
    );

    user_wallpaper(plan, user_elevate, user, &user_directories);
    user_services(plan, user_elevate, user, &user_directories, machine);

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
            content: FileContent::Lines(Ref(&[Ref(MY_SSH_PUBKEY)])),
        },
    );

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

    Ok(UserResult { packages })
}

fn user_create_directories<'elevate: 'action, 'action, 'user, 'machine>(
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

fn user_autologin<'elevate: 'action, 'action, 'user, 'machine>(
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
                content: Lines(Owned(vec![
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

fn user_dotfiles<'elevate: 'action, 'action, 'user, 'machine, 'pkgs>(
    plan: &mut Block<'_, 'elevate, 'pkgs, 'action, '_, Secrets>,
    user_elevate: &'elevate Elevate,
    user: &'user UserConfig,
    machine: &MachineConfig,
    home: &Path,
    directories: &UserDirectories,
) -> Result<PackageList<'pkgs>> {
    #[derive(Serialize)]
    struct TemplateContext<'a> {
        user: &'a UserConfig,
        machine: &'a MachineConfig,
        is_laptop: bool,
        is_workstation: bool,
        is_tv: bool,
    }

    let mut packages = HashMap::new();

    let template_context = TemplateContext {
        user,
        machine,
        is_laptop: machine.machine_type == Laptop,
        is_workstation: machine.machine_type == Workstation,
        is_tv: machine.machine_type == Tv,
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
                    "/../files/autostart/services/sway.service"
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

        plan.add(
            Ref("add github_light theme for alacritty"),
            user_elevate,
            File {
                path: Owned(themes_dir.join("github_light.toml")),
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
                    "/../files/alacritty/themes/github_light.toml"
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

        let service_path = directories.systemd_user_units.join("gpg-agent.service");
        if user.gpg.as_ref().is_some_and(|gpg| gpg.agent_enabled) {
            plan.add(
                Ref("add gpg-agent service"),
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
                        "/../files/autostart/services/gpg-agent.service"
                    )))),
                },
            );
        } else {
            plan.add(
                Ref("user service gpg-agent"),
                user_elevate,
                Remove {
                    path: Owned(service_path),
                    on_directory: DirectoryHandling::Bail,
                    on_file: FileHandling::Proceed,
                    on_symlink: SymlinkHandling::Proceed,
                },
            );
        }

        Ok(())
    })?;

    plan.block("rust", |plan| {
        let config_dir = directories.state.join("cargo");

        packages.insert(
            Ref("rust"),
            Ref([
                Ref("rustup"),
                Ref("rust-analyzer"),
                Ref("cargo-edit"),
                Ref("cargo-expand"),
                Ref("cargo-msrv"),
                Ref("cargo-watch"),
                Ref("cargo-release"),
                Ref("cargo-sort"),
                Ref("cargo-hack"),
                Ref("cargo-dist"),
                Ref("cargo-binstall"),
                Ref("cargo-machete"),
                Ref("rust-script"),
                Ref("bacon"),
                Ref("mold"),
            ]
            .as_slice()),
        );

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
        plan.add(
            Ref("add gnome-keyring service"),
            user_elevate,
            File {
                path: Owned(directories.systemd_user_units.join("gnome-keyring.service")),
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
                    "/../files/autostart/services/gnome-keyring.service"
                )))),
            },
        );
    });

    plan.block("kdeconnect", |plan| {
        packages.insert(Ref("kdeconnect"), Ref([Ref("kdeconnect")].as_slice()));
        plan.add(
            Ref("add kdeconnect service"),
            user_elevate,
            File {
                path: Owned(directories.systemd_user_units.join("kdeconnect.service")),
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
                    "/../files/autostart/services/kdeconnect.service"
                )))),
            },
        );
    });

    plan.block("keepassxc", |plan| {
        packages.insert(Ref("keepassxc"), Ref([Ref("keepassxc")].as_slice()));

        let service_path = directories.systemd_user_units.join("keepassxc.service");
        if user.keepassxc.enabled() {
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
                        "/../files/autostart/services/keepassxc.service"
                    )))),
                },
            );
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
        packages.insert(Ref("nextloud"), Ref([Ref("nextcloud-client")].as_slice()));

        let service_path = directories.systemd_user_units.join("nextcloud.service");
        if user.nextcloud.enabled() {
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
                        "/../files/autostart/services/nextcloud.service"
                    )))),
                },
            );
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
        plan.add(
            Ref("add screencfg service"),
            user_elevate,
            File {
                path: Owned(directories.systemd_user_units.join("screencfg.service")),
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
                    "/../files/autostart/services/screencfg.service"
                )))),
            },
        );
    });

    plan.block("spotify", |plan| {
        plan.add(
            Ref("add spotify service"),
            user_elevate,
            File {
                path: Owned(directories.systemd_user_units.join("spotify.service")),
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
                    "/../files/autostart/services/spotify.service"
                )))),
            },
        );
    });

    plan.block("discord", |plan| {
        packages.insert(Ref("discord"), Ref(&[Ref("discord")]));

        let service_path = directories.systemd_user_units.join("discord.service");
        if machine.roles.has(MachineRole::Gaming) {
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
                        "/../files/autostart/services/discord.service"
                    )))),
                },
            );
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

        let service_path = directories.systemd_user_units.join("steam.service");
        if user.steam.enabled() {
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
                        "/../files/autostart/services/steam.service"
                    )))),
                },
            );
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

    plan.block("yubikey-touch-detector", |plan| {
        let service_path = directories
            .systemd_user_units
            .join("yubikey-touch-detector.service");
        if user.gpg.as_ref().is_some_and(|gpg| gpg.agent_enabled) {
            plan.add(
                Ref("add yubikey-touch-detector service"),
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
                        "/../files/autostart/services/yubikey-touch-detector.service"
                    )))),
                },
            );
        } else {
            plan.add(
                Ref("user service yubikey-touch-detector"),
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
        let service_path = directories.systemd_user_units.join("laptop-lid.service");
        if machine.machine_type == MachineType::Laptop {
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
                        "/../files/autostart/services/laptop-lid.service"
                    )))),
                },
            );
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
        packages.insert(Ref("gammastep"), Ref([Ref("gammastep")].as_slice()));

        plan.add(
            Ref("add gammastep config"),
            user_elevate,
            File {
                path: Owned(directories.config.join("gammastep.conf")),
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

        let service_path = directories.systemd_user_units.join("gammastep.service");
        if machine.machine_type != MachineType::Tv {
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
                        "/../files/autostart/services/gammastep.service"
                    )))),
                },
            );
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
        }
    });

    plan.block("workstation-mgr", |plan| {
        plan.add(
            Ref("add workstation-mgr service"),
            user_elevate,
            File {
                path: Owned(
                    directories
                        .systemd_user_units
                        .join("workstation-mgr.service"),
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
                content: FileContent::Bytes(Ref(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/autostart/services/workstation-mgr.service"
                )))),
            },
        );
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

        plan.add(
            Ref("add dunst service"),
            user_elevate,
            File {
                path: Owned(directories.systemd_user_units.join("dunst.service")),
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
                    "/../files/autostart/services/dunst.service"
                )))),
            },
        );

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

        for profile in &user.firefox.profiles {
            let profile_dir = firefox_dir.join(format!("profile-{}", profile.name));
            let chrome_dir = profile_dir.join("chrome");

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
                    "/../files/autostart/services/firefox@.service"
                )))),
            },
        );
    });

    Ok(packages)
}

async fn user_executables<'elevate: 'action, 'action, 'user, 'machine, 'pkgs>(
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

fn user_wallpaper<'elevate: 'action, 'action, 'user, 'machine>(
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
fn user_services<'elevate: 'action, 'action, 'user, 'machine>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
    user_elevate: &'elevate Elevate,
    user: &'user UserConfig,
    user_directories: &UserDirectories,
    machine: &MachineConfig,
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
                    content: FileContent::Bytes(Ref(include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../files/autostart/services/windowmanager.target"
                    )))),
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
                        let mut out = vec![
                            Ref("[Unit]"),
                            Ref("BindsTo=windowmanager.target"),
                            Ref("After=windowmanager.target"),
                            Ref(""),
                            Ref("Wants=gnome-keyring.service"),
                            Ref("Wants=dunst.service"),
                            Ref("Wants=nm-sidebar.service"),
                            Ref("Wants=gammastep.service"),
                            Ref("Wants=spotify.service"),
                            Ref("Wants=kdeconnect.service"),
                            Ref("Wants=color-theme-dark.service"),
                            Ref("Wants=workstation-mgr.service"),
                            Ref("Wants=screencfg.service"),
                        ];

                        if user.gpg.as_ref().is_some_and(|gpg| gpg.agent_enabled) {
                            out.extend([
                                Ref("Wants=gpg-agent.service"),
                                Ref("Wants=yubikey-touch-detector.service"),
                            ]);
                        }

                        if user.keepassxc.enabled() {
                            out.push(Ref("Wants=keepassxc.service"));
                        }

                        if machine.machine_type == Laptop {
                            out.push(Ref("Wants=laptop-lid.service"));
                        }

                        if user.nextcloud.enabled() {
                            out.push(Ref("Wants=nextcloud.service"));
                        }

                        if machine.roles.has(MachineRole::Gaming) {
                            out.push(Ref("Wants=discord.service"));
                        }

                        if user.steam.enabled() {
                            out.push(Ref("Wants=steam.service"));
                        }

                        for profile in &user.firefox.profiles {
                            out.push(Owned(format!(
                                "Wants=firefox@{name}.service",
                                name = profile.name
                            )));
                        }

                        out
                    })),
                },
            );
        });

        for (unit, content) in [
            (
                "color-theme-dark.service",
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/services/color-theme-dark.service"
                ))
                .as_slice(),
            ),
            (
                "color-theme-light.service",
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../files/services/color-theme-light.service"
                ))
                .as_slice(),
            ),
        ] {
            plan.add(
                Owned(format!("user service {unit}")),
                user_elevate,
                File {
                    path: Owned(user_directories.systemd_user_units.join(unit)),
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
                    content: FileContent::Bytes(Ref(content)),
                },
            );
        }
    });
}

#[derive(Eq, PartialEq, Serialize)]
enum MachineType {
    Tv,
    Workstation,
    Laptop,
}
use MachineType::{Laptop, Tv, Workstation};

#[derive(Serialize)]
struct MachineRoles(&'static [MachineRole]);

impl MachineRoles {
    fn has(&self, role: MachineRole) -> bool {
        self.0.contains(&role)
    }
}

#[derive(Eq, PartialEq, Serialize, Clone, Copy)]
enum MachineRole {
    Gaming,
}

#[derive(Serialize)]
struct WorkspaceConfig {
    num1: Option<String>,
    num2: Option<String>,
    num3: Option<String>,
    num4: Option<String>,
    num5: Option<String>,
    num6: Option<String>,
    num7: Option<String>,
    num8: Option<String>,
    num9: Option<String>,
    num0: Option<String>,
}

#[derive(Serialize, VariantArray)]
enum Language {
    En,
    De,
}

impl Language {
    fn as_locale(&self) -> &str {
        match *self {
            Self::En => "en_US.UTF-8",
            Self::De => "de_DE.UTF-8",
        }
    }
}

#[derive(Serialize)]
struct MachineConfig {
    hostname: &'static str,
    encrypted_root: bool,
    boot_target: Target,
    cpu: Cpu,
    gpu: Gpu,
    font_size: usize,
    workspace: WorkspaceConfig,
    machine_type: MachineType,
    roles: MachineRoles,
    language: Language,
}

#[derive(Serialize)]
struct UserKeyboardConfig {
    repeat_delay: usize,
    repeat_rate: usize,
}

#[derive(Serialize)]
struct UserGpgConfig {
    email: String,
    id: String,
    fingerprint: String,
    gpg_agent_for_ssh: bool,
    agent_enabled: bool,
}

#[derive(Serialize)]
struct FirefoxProfile {
    name: String,
    manage_css: bool,
    extensions: Vec<String>,
}

#[derive(Serialize)]
struct FirefoxUserConfig {
    profiles: Vec<FirefoxProfile>,
}

#[derive(PartialEq, Eq, Serialize)]
enum FeatureState {
    Enabled,
    Disabled,
}

impl FeatureState {
    fn enabled(&self) -> bool {
        *self == Self::Enabled
    }
}

#[derive(Serialize)]
struct UserConfig {
    name: String,
    fullname: String,
    mail: String,
    keyboard: UserKeyboardConfig,
    ssh_agent_enabled: bool,
    gpg: Option<UserGpgConfig>,
    vt: usize,
    nextcloud: FeatureState,
    steam: FeatureState,
    keepassxc: FeatureState,
    autologin: bool,
    firefox: FirefoxUserConfig,
}

struct UserContext {
    simplest_elevate: Elevate,
    strict_elevate: Elevate,
    config: UserConfig,
}

struct Config {
    machine: MachineConfig,
    users: Vec<UserContext>,
}

fn configure_tracing(level: tracing::Level) -> impl tracing::Subscriber {
    tracing_subscriber::fmt()
        .event_format(tracing_subscriber::fmt::format().compact())
        .with_timer(tracing_subscriber::fmt::time::SystemTime)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE)
        .with_ansi(true)
        .with_max_level(level)
        .with_target(true)
        .finish()
}

fn private_user(
    autologin: bool,
    vt: usize,
    name: String,
    steam: FeatureState,
    machine: &MachineConfig,
) -> UserConfig {
    UserConfig {
        autologin,
        vt,
        name,
        fullname: "Hannes Körber".to_owned(),
        mail: "hannes@hkoerber.de".to_owned(),
        keyboard: if machine.machine_type == Tv {
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
            email: "hannes@hkoerber.de".to_owned(),
            id: "0xB5C002530C6A2053".to_owned(),
            fingerprint: "973AE48D71B76735C4712B5BB5C002530C6A2053".to_owned(),
            gpg_agent_for_ssh: true,
        }),
        nextcloud: FeatureState::Enabled,
        steam,
        keepassxc: FeatureState::Disabled,
        firefox: FirefoxUserConfig {
            profiles: vec![FirefoxProfile {
                name: "default".to_owned(),
                manage_css: true,
                extensions: vec![
                    "ublock-origin".to_owned(),
                    "passff".to_owned(),
                    "tree-style-tab".to_owned(),
                    "i-dont-care-about-cookies".to_owned(),
                    "floccus".to_owned(),
                ],
            }],
        },
    }
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

enum MainResult {
    Success,
    Failure,
    Error(anyhow::Error),
}

#[expect(clippy::print_stderr, reason = "we need to report the error")]
impl process::Termination for MainResult {
    fn report(self) -> process::ExitCode {
        match self {
            Self::Success => process::ExitCode::SUCCESS,
            Self::Failure => process::ExitCode::FAILURE,
            Self::Error(e) => {
                eprintln!("Error: {e}");
                process::ExitCode::FAILURE
            }
        }
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> MainResult {
    async fn inner() -> Result<std::result::Result<(), ()>> {
        let args = Cli::try_parse()?;

        let subscriber = configure_tracing(if args.debug {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        });

        tracing::subscriber::set_global_default(subscriber)?;

        let running_user = Process::run("id", &["--user", "--name"], &Elevate::No, Stdin::Closed)
            .await?
            .check()?
            .stdout
            .trim()
            .to_owned();

        match args.action {
            #[expect(
                clippy::print_stdout,
                clippy::print_stderr,
                reason = "migrate is more chatty"
            )]
            Action::Migrate { dry_run, init } => {
                let root = match running_user.as_str() {
                    "root" => Elevate::No,
                    _ => Elevate::Sudo(ElevateTo::Root),
                };

                let state = migrations::MachineState::load(&root).await?;

                match state {
                    Some(mut state) => {
                        if init {
                            eprintln!("state already present, --init cannot be done");
                            Ok(Err(()))
                        } else {
                            let any_applied = state.apply_migrations(&root, dry_run).await?;
                            if any_applied {
                                println!("All migrations applied");
                            }

                            Ok(Ok(()))
                        }
                    }
                    None => {
                        if init {
                            let id = migrations::Migration::latest();
                            let new_state = migrations::MachineState::new(id);
                            if dry_run {
                                println!("would save new state {new_state}");
                            } else {
                                new_state.save(&root).await?;
                            }
                            Ok(Ok(()))
                        } else {
                            Err(anyhow!("no state found, use --init to initialize it"))
                        }
                    }
                }
            }
            Action::Deploy {
                dry_run,
                only_changed,
                clean_packages,
                secrets_stdin,
            } => {
                let root = match running_user.as_str() {
                    "root" => Elevate::No,
                    _ => Elevate::Sudo(ElevateTo::Root),
                };

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
                                extensions: vec![
                                    "ublock-origin".to_owned(),
                                    "tree-style-tab".to_owned(),
                                    "i-dont-care-about-cookies".to_owned(),
                                ],
                            },
                            FirefoxProfile {
                                name: "meet".to_owned(),
                                manage_css: false,
                                extensions: vec!["ublock-origin".to_owned()],
                            },
                        ],
                    },
                };

                let machine = match args.hostname {
                    Neptune => MachineConfig {
                        hostname: args.hostname.as_str(),
                        encrypted_root: true,
                        boot_target: Target::MultiUser,
                        cpu: Cpu::Intel,
                        gpu: Gpu::Nvidia,
                        font_size: 11,
                        workspace: WorkspaceConfig {
                            num1: Some("".to_owned()),
                            num2: Some("".to_owned()),
                            num3: Some("".to_owned()),
                            num4: Some("".to_owned()),
                            num5: None,
                            num6: None,
                            num7: Some("".to_owned()),
                            num8: Some("".to_owned()),
                            num9: Some("".to_owned()),
                            num0: Some("".to_owned()),
                        },
                        machine_type: Laptop,
                        roles: MachineRoles(&[]),
                        language: Language::En,
                    },
                    Ares => MachineConfig {
                        hostname: args.hostname.as_str(),
                        encrypted_root: true,
                        boot_target: Target::MultiUser,
                        cpu: Cpu::Amd,
                        gpu: Gpu::Amd,
                        font_size: 10,
                        workspace: WorkspaceConfig {
                            num1: Some("".to_owned()),
                            num2: Some("".to_owned()),
                            num3: Some("".to_owned()),
                            num4: Some("".to_owned()),
                            num5: None,
                            num6: None,
                            num7: None,
                            num8: Some(" ".to_owned()),
                            num9: Some("".to_owned()),
                            num0: Some("".to_owned()),
                        },
                        machine_type: Workstation,
                        roles: MachineRoles(&[MachineRole::Gaming]),
                        language: Language::En,
                    },
                    Hera => MachineConfig {
                        hostname: args.hostname.as_str(),
                        encrypted_root: false,
                        boot_target: Target::Graphical,
                        cpu: Cpu::Intel,
                        gpu: Gpu::Intel,
                        font_size: 11,
                        workspace: WorkspaceConfig {
                            num1: Some("".to_owned()),
                            num2: Some("".to_owned()),
                            num3: Some("".to_owned()),
                            num4: Some("".to_owned()),
                            num5: None,
                            num6: None,
                            num7: None,
                            num8: None,
                            num9: Some("".to_owned()),
                            num0: Some("".to_owned()),
                        },
                        machine_type: Laptop,
                        roles: MachineRoles(&[]),
                        language: Language::De,
                    },
                    Dionysus => MachineConfig {
                        hostname: args.hostname.as_str(),
                        encrypted_root: true,
                        boot_target: Target::MultiUser,
                        cpu: Cpu::Intel,
                        gpu: Gpu::Intel,
                        font_size: 16,
                        workspace: WorkspaceConfig {
                            num1: Some("".to_owned()),
                            num2: Some("".to_owned()),
                            num3: Some("".to_owned()),
                            num4: None,
                            num5: None,
                            num6: None,
                            num7: None,
                            num8: None,
                            num9: None,
                            num0: None,
                        },
                        machine_type: Tv,
                        roles: MachineRoles(&[]),
                        language: Language::En,
                    },
                };

                let users = match args.hostname {
                    Neptune => vec![
                        private_user(
                            true,
                            2,
                            "hannes-private".to_owned(),
                            FeatureState::Disabled,
                            &machine,
                        ),
                        work_user,
                    ],
                    Ares => vec![private_user(
                        true,
                        1,
                        "hannes".to_owned(),
                        FeatureState::Enabled,
                        &machine,
                    )],
                    Dionysus => vec![private_user(
                        true,
                        1,
                        "hannes".to_owned(),
                        FeatureState::Disabled,
                        &machine,
                    )],
                    Hera => vec![private_user(
                        false,
                        1,
                        "hannes".to_owned(),
                        FeatureState::Disabled,
                        &machine,
                    )],
                }
                .into_iter()
                .map(|config| UserContext {
                    simplest_elevate: match running_user.as_str() {
                        "root" => Elevate::No,
                        name if name == config.name => Elevate::No,
                        _other => Elevate::Sudo(ElevateTo::User {
                            user: config.name.clone(),
                        }),
                    },
                    strict_elevate: match running_user.as_str() {
                        name if name == config.name => Elevate::No,
                        _other => Elevate::Sudo(ElevateTo::User {
                            user: config.name.clone(),
                        }),
                    },
                    config,
                })
                .collect();

                let config = Config { machine, users };

                let secrets = if secrets_stdin {
                    Some(Secrets::load_from(io::stdin())?)
                } else {
                    None
                };

                let mut plan = Plan::new();

                if let Some(ref secrets) = secrets {
                    plan.set_secrets(SecretsWorkflow::Secrets(secrets));
                } else {
                    plan.set_secrets(SecretsWorkflow::AssumeExisting);
                }

                root_setup(&mut plan, &root, &config)?;

                let mut user_packages: HashMap<
                    MaybeOwned<'_, str>,
                    MaybeOwned<'_, [MaybeOwned<'_, str>]>,
                > = HashMap::new();

                for user in &config.users {
                    plan.try_block_async(Owned(user.config.name.clone()), async |plan| {
                        let user_result =
                            configure_user(plan, &root, user, &config.machine).await?;
                        user_packages.extend(user_result.packages);

                        Ok(())
                    })
                    .await?;
                }

                plan.block("user packages", |plan| {
                    #[expect(clippy::iter_over_hash_type, reason = "order does not matter")]
                    for (section, packages) in user_packages {
                        plan.add(
                            Owned(format!(
                                "install user packages for {section}",
                                section = section.as_ref()
                            )),
                            &root,
                            Packages {
                                install: packages.clone(),
                                reason: InstallReason::Explicit,
                            },
                        );

                        plan.register_packages(packages.as_ref());
                    }
                });

                plan.block("application packages", |plan| {
                    for &(section, packages) in APPLICATION_PACKAGES {
                        let packages = packages
                            .iter()
                            .map(|package| Ref(*package))
                            .collect::<Vec<_>>();

                        plan.add(
                            Owned(format!("install packages for {section}")),
                            &root,
                            Packages {
                                install: Owned(packages.clone()),
                                reason: InstallReason::Explicit,
                            },
                        );

                        plan.register_packages(&packages);
                    }
                });

                aur(&mut plan, &root);

                match args.hostname {
                    Hera => {
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
                                &root,
                                Packages {
                                    install: Ref(packages),
                                    reason: InstallReason::Explicit,
                                },
                            );

                            plan.register_packages(packages);

                            plan.add(
                                Ref("create user"),
                                &root,
                                User {
                                    name: "herta",
                                    home: Ref(Path::new("/home/herta")),
                                    primary_group: PrimaryGroupConfig::Automatic,
                                    additional_groups: &[
                                        "wheel", "sudonopw", "games", "kvm", "video",
                                    ],
                                    shell: "/usr/bin/zsh",
                                    system: false,
                                },
                            );

                            plan.block("login manager", |plan| {
                                plan.add(
                                    Ref("enable plasma-login-manager"),
                                    &root,
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
                                    &root,
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
                                    &root,
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

                            autoupdate(plan, &root);
                            restic_backup(plan, &root);
                        });
                    }
                    Dionysus => {
                        plan.block("dionysus", |plan| {
                            autoupdate(plan, &root);
                        });
                    }
                    Ares | Neptune => (),
                }

                let plan_finished = match plan.finish().await? {
                    FinishResult::AssertionsFailed { report } => {
                        report.print();
                        return Ok(Err(()));
                    }
                    FinishResult::Success(plan) => plan,
                };

                let report = plan_finished.report()?;
                report.print(
                    &if only_changed {
                        ReportConfig::OnlyChanged
                    } else {
                        ReportConfig::All
                    },
                    true,
                );

                let has_changes = report.has_changes();

                let plan_approved = plan_finished.approve();

                if !dry_run && has_changes {
                    plan_approved.apply().await?;
                }

                if !dry_run && clean_packages {
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
                        let managed_packages = plan_approved.managed_packages();

                        unmanaged_packages(managed_packages, &mut unmanaged_package_plan, &root)
                            .await?;

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

                        orphans(&mut orphaned_package_plan, &root).await?;

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
                }

                Ok(Ok(()))
            }
            Action::Bootstrap { i_am_sure } => {
                if running_user != "root" {
                    Err(anyhow!("needs to be run as root"))
                } else if i_am_sure {
                    let elevate = Elevate::No;
                    let tempfile = Tempfile::new(&elevate).await?;
                    tempfile
                        .write(match args.hostname {
                            Neptune => include_bytes!(concat!(
                                env!("CARGO_MANIFEST_DIR"),
                                "/../install_scripts/neptune.sh"
                            )),
                            Ares => include_bytes!(concat!(
                                env!("CARGO_MANIFEST_DIR"),
                                "/../install_scripts/ares.sh"
                            )),
                            Dionysus => include_bytes!(concat!(
                                env!("CARGO_MANIFEST_DIR"),
                                "/../install_scripts/dionysus.sh"
                            )),
                            Hera => include_bytes!(concat!(
                                env!("CARGO_MANIFEST_DIR"),
                                "/../install_scripts/hera.sh"
                            )),
                        })
                        .await?;

                    Process::run_in_foreground(
                        "bash",
                        &[tempfile.path().as_str()],
                        &elevate,
                        &Stdin::Open,
                    )?
                    .check()?;
                    Ok(Ok(()))
                } else {
                    Err(anyhow!("coward"))
                }
            }
        }
    }

    match inner().await {
        Ok(Ok(())) => MainResult::Success,
        Ok(Err(())) => MainResult::Failure,
        Err(err) => MainResult::Error(err),
    }
}
