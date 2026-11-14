#![expect(
    clippy::large_include_file,
    reason = "Everything necessary should be included in the binary (PKGBUILDS, locally built packages)"
)]
#![allow(unused, reason = "dev")]
#![deny(unused_must_use)]
// required for `serde_json::json!()`
#![recursion_limit = "256"]

use std::{env, io, net::Ipv4Addr, process};

use anyhow::{Result, anyhow, bail};
use clap::{Parser, Subcommand, ValueEnum};
use dedent::dedent;
use include_dir::{DirEntry, include_dir};
use regex::Regex;
use serde::{Deserialize, Serialize};

use cfg::prelude::*;

static BINS: include_dir::Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../files/bin");

static SCRIPTS: include_dir::Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../files/scripts");

static INCLUDED_PACKAGES: &[(&str, &str, &str, &[u8])] =
    include!(concat!(env!("OUT_DIR"), "/packages.rs"));

const MY_SSH_PUBKEY: &str = "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAACAQCxOtVxz3vRL7SUZVO899RUcg9BiDuZfPUfsd9v3AHOwNvSVsLObqq0QgwajmTML9nRZzX7kc7RmFtXZ5ypMkyEJ9p14rYXKYKhLqEM4Paz0lUagQhy0sdYijtayNsM5luU3yk6a4F/PkanprDGEEtJPMLCwpDODTwlE7THvFtJXLnYeFtF2DJKnUcP6E9MWGYM1zYdrmf43pxWg2YXFjoyLEkOy6TVjjqwz+tKa6+4YRqFWNyXcEWqI2ZLqHRzZTvu+46gsnR+8HcLpov3ij/Mr0jYSJ4vqMGz1yfKnrsKZUp0IhvlPYbGjdcL17xYame2UroPS8AzWWegyqeEFh54qmCrFVw/uwErcWcUJX3KEijrc48DYZUBK09vnFFcXIYR/cfFWpTGhnqlwp/EAaBeR3zr6hS6lL+XluMIjHHkwT/eD3YydEwABjbJwBJp5PdYR4M/bSHuOoCH6d/U36OpkwwekyYKWvEoxxPSJIWCy1ApILQZmWuctIFc8SQfSXfRifPEDBnL9hNzzX00beDYmS6cPIP0TIqa9fdL1AeO68Tk7o67f2cCE11QTH7FI9JjhuQ/M4dO8jUw7kmtGCKdReNIuYYU3xZB4wdFHs/5UxROCwEZnv+3uWAgHbgc14XLWunN9P9msOxYLHPVJK/XcdhHvWSmQdNq4ajry/H+kQ==";

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

use Host::{Ares, Dionysus, Hera, Neptune};

#[derive(Deserialize)]
struct Secret<T>(T);

impl<T> std::fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("[redacted]").finish()
    }
}

impl<T> Secret<T> {
    fn access(&self) -> &T {
        &self.0
    }
}

#[derive(Deserialize)]
struct WireguardPubkey(String);

impl WireguardPubkey {
    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize)]
struct WireguardPrivkey(String);

impl WireguardPrivkey {
    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize)]
struct WireguardPsk(String);

impl WireguardPsk {
    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize)]
struct Domain(String);

impl Domain {
    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize)]
struct Port(u16);

impl Port {
    fn as_inner(&self) -> u16 {
        self.0
    }
}

#[derive(Debug, Deserialize)]
struct WireguardSecrets {
    address: Secret<Ipv4Addr>,
    privkey: Secret<WireguardPrivkey>,
    psk: Secret<WireguardPsk>,
    server_pubkey: Secret<WireguardPubkey>,
    server_ip: Secret<Ipv4Addr>,
    server_endpoint_domain: Secret<Domain>,
    server_endpoint_port: Secret<Port>,
}

#[derive(Debug, Deserialize)]
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

struct PackageConfig(&'static [(&'static str, &'static [&'static str])]);

const PACKAGES: PackageConfig = PackageConfig(&[
    (
        "kernel",
        &[
            "linux-zen",
            "linux-zen-headers",
            "linux-zen-docs",
            "linux-firmware",
        ],
    ),
    ("grub", &["grub", "efibootmgr"]),
    ("lvm", &["lvm2"]),
    (
        "build-essentials",
        &["gcc", "ctags", "cmake", "maven", "base-devel"],
    ),
    ("posix", &["posix"]),
    ("make", &["make"]),
    ("gdb", &["gdb"]),
    ("strace", &["strace"]),
    ("sudo", &["sudo"]),
    ("doas", &["opendoas"]),
    ("ansible", &["ansible", "ansible-lint", "python-jmespath"]),
    ("awesome-terminal-fonts", &["awesome-terminal-fonts"]),
    (
        "font-inconsolata",
        &["ttf-inconsolata", "ttf-inconsolata-nerd"],
    ),
    (
        "font-jetbrainsmono",
        &["ttf-jetbrains-mono", "ttf-jetbrains-mono-nerd"],
    ),
    ("font-dejavu", &["ttf-dejavu", "ttf-dejavu-nerd"]),
    ("font-libertine", &["otf-libertinus"]),
    ("font-awesome", &["woff2-font-awesome"]),
    ("font-noto", &["noto-fonts"]),
    ("git", &["git", "qgit", "git-delta", "git-filter-repo"]),
    ("htop", &["htop"]),
    ("neovim", &["neovim", "python-pynvim"]),
    ("vim", &["vim"]),
    ("gammastep", &["gammastep"]),
    ("pinentry-qt", &["pinentry"]),
    ("pass", &["pass", "passff-host"]),
    ("alacritty", &["alacritty"]),
    ("tmux", &["tmux"]),
    (
        "zsh",
        &["zsh", "zsh-syntax-highlighting", "zsh-completions"],
    ),
    ("zathura", &["zathura", "zathura-pdf-poppler"]),
    ("pdf", &["ghostscript", "enscript"]),
    (
        "pandoc",
        &[
            "pandoc-cli",
            "texlive-basic",
            "texlive-fontsextra",
            "texlive-latexextra",
        ],
    ),
    (
        "libvirt",
        &[
            "virt-manager",
            "libvirt",
            "dnsmasq",
            "dmidecode",
            "virt-install",
            "virt-viewer",
            "libguestfs",
            "edk2-ovmf",
        ],
    ),
    (
        "firefox",
        &[
            "firefox",
            "pipewire-media-session", // required for screensharing in wayland
        ],
    ),
    ("ranger", &["ranger"]),
    ("thunar", &["thunar"]),
    (
        "libreoffice",
        &["libreoffice-fresh", "libreoffice-fresh-de"],
    ),
    (
        "qt-theming",
        &["breeze", "breeze-icons", "breeze-grub", "qt5ct"],
    ),
    ("gtk-theming", &["breeze-gtk", "lxappearance"]),
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
    ("python-modules", &["python-ruamel-yaml"]),
    ("brightnessctl", &["brightnessctl"]),
    ("wireshark", &["wireshark-cli", "wireshark-qt"]),
    ("nmap", &["nmap"]),
    ("curl", &["curl"]),
    ("wget", &["wget"]),
    ("tree", &["tree"]),
    ("which", &["which"]),
    ("zip", &["zip", "unzip"]),
    ("traceroute", &["traceroute"]),
    ("tcpdump", &["tcpdump"]),
    ("tar", &["tar"]),
    ("rsync", &["rsync"]),
    ("net-tools", &["net-tools"]),
    ("ntfs", &["ntfs-3g"]),
    ("lsof", &["lsof"]),
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
    ("networkmanager", &["networkmanager", "capnet-assist"]),
    (
        "pulseaudio",
        &[
            "pulseaudio",
            "pulseaudio-alsa",
            "lib32-libpulse",
            "alsa-utils",
            "alsa-firmware",
            "sof-firmware",
            "alsa-ucm-conf",
        ],
    ),
    // this needs to come *after* pulseaudio, otherwise it will pull in pipewire-pulse as the
    // provider for pulse-native-provider
    ("pavucontrol", &["pavucontrol-qt"]),
    ("iw", &["iw"]),
    ("cowsay", &["cowsay"]),
    ("ruby", &["ruby"]),
    ("acpi", &["acpi", "acpid"]),
    ("nodejs", &["nodejs-lts-jod", "npm", "yarn"]),
    // this needs to come *after* nodejs, otherwise dockerfile-language-server will pull
    // nodejs (non-LTS)
    ("docker", &["dockerfile-language-server"]),
    ("xdg", &["xdg-utils"]),
    ("dunst", &["dunst"]),
    ("bwm-ng", &["bwm-ng"]),
    ("ssh", &["openssh"]),
    ("sshfs", &["sshfs"]),
    ("expect", &["expect"]),
    ("inotify", &["inotify-tools"]),
    ("rclone", &["rclone"]),
    (
        "rust",
        &[
            "rustup",
            "rust-analyzer",
            "cargo-edit",
            "cargo-expand",
            "cargo-msrv",
            "cargo-watch",
            "cargo-release",
            "cargo-sort",
            "cargo-hack",
            "cargo-dist",
            "cargo-binstall",
            "cargo-machete",
            "rust-script",
            "bacon",
        ],
    ),
    ("musescore", &["musescore"]),
    ("rofi", &["rofi"]),
    ("imv", &["imv"]),
    ("pacman", &["pacman", "pacman-contrib"]),
    (
        "steam",
        &["steam", "lib32-pango", "pango", "ttf-liberation"],
    ),
    ("man", &["man-db", "man-pages"]),
    ("nextcloud", &["nextcloud-client"]),
    ("kwallet", &["kwallet", "kwalletmanager"]),
    ("pv", &["pv"]),
    ("stress", &["stress"]),
    ("mpris", &["playerctl"]),
    ("imagemagick", &["imagemagick"]),
    ("mpv", &["mpv"]),
    ("gnome-keyring", &["gnome-keyring", "seahorse"]),
    ("mkinitcpio", &["mkinitcpio"]),
    ("terraform", &["terraform", "terragrunt"]),
    ("ncdu", &["ncdu"]),
    ("dust", &["dust"]),
    ("font-utils", &["woff2"]),
    ("jq", &["jq"]),
    ("kdeconnect", &["kdeconnect"]),
    ("restic", &["restic"]),
    ("discord", &["discord"]),
    ("kubectl", &["kubectl"]),
    ("bind", &["bind"]),
    ("fzf", &["fzf"]),
    ("chromium", &["chromium"]),
    ("go", &["go", "gopls", "delve", "golangci-lint"]),
    ("helix", &["helix"]),
    ("keepassxc", &["keepassxc"]),
    ("awscli", &["aws-cli-v2"]),
    ("mariadb-client", &["mariadb-clients"]),
    ("php", &["php", "composer"]),
    ("eza", &["eza"]),
    ("just", &["just", "just-lsp"]),
    ("ripgrep", &["ripgrep"]),
    ("fd", &["fd"]),
    ("bat", &["bat"]),
    ("mdbook", &["mdbook"]),
    (
        "bash",
        &["bash", "bash-language-server", "shellcheck", "shfmt"],
    ),
    ("packer", &["packer"]),
    ("c", &["gcc", "clang"]),
    ("sed", &["sed"]),
    ("findutils", &["findutils"]),
    ("html", &["vscode-html-languageserver"]),
    ("typescript", &["typescript", "typescript-language-server"]),
    ("json", &["vscode-json-languageserver", "gron"]),
    ("markdown", &["marksman", "mdformat"]),
    ("lldb", &["lldb"]),
    ("yaml", &["yaml-language-server", "yamllint"]),
    ("qemu", &["qemu-base", "qemu-desktop"]),
    ("cloud-init", &["cloud-init", "cloud-utils"]),
    ("netcat", &["openbsd-netcat"]),
    ("telnet", &["inetutils"]),
    ("cloudformation-tools", &["python-cfn-lint"]),
    ("johntheripper", &["john"]),
    ("age", &["age"]),
    ("httpie", &["httpie"]),
    ("yt-dlp", &["yt-dlp"]),
    ("ytfzf", &["ytfzf"]),
    ("ffmpeg", &["ffmpeg"]),
    ("kcharselect", &["kcharselect"]),
    ("sysstat", &["sysstat"]),
    ("qrencode", &["qrencode"]),
    ("iotop", &["iotop"]),
    ("w3m", &["w3m"]),
    ("ruff", &["ruff"]),
    ("mold", &["clang", "mold"]),
    ("wild", &["wild"]),
    (
        "arch-packaging",
        &["namcap", "devtools", "arch-install-scripts", "archiso"],
    ),
    ("sqlite", &["sqlite"]),
    ("css", &["vscode-css-languageserver"]),
    ("watchexec", &["watchexec"]),
    ("postgresql", &["postgresql", "pgformatter"]),
    ("tokei", &["tokei"]),
    ("whois", &["whois"]),
    ("encfs", &["encfs"]),
    ("digikam", &["digikam"]),
    ("evince", &["evince"]),
    ("iftop", &["iftop"]),
    ("inkscape", &["inkscape"]),
    ("android", &["android-tools", "android-udev"]),
    ("audacity", &["audacity"]),
    ("calibre", &["calibre"]),
    ("deluge", &["deluge", "deluge-gtk"]),
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
    ("sqlx", &["sqlx-cli"]),
    ("geeqie", &["geeqie"]),
    ("xpath", &["perl-xml-xpath"]),
    ("apache", &["apache"]),
    ("xfce4-screenshooter", &["xfce4-screenshooter"]),
    ("fuseiso", &["fuseiso"]),
    ("7zip", &["7zip"]),
    ("unrar", &["unrar"]),
    ("toml", &["taplo-cli", "tombi"]),
    ("prettier", &["prettier"]),
    ("btop", &["btop"]),
    ("dua", &["dua-cli"]),
    ("hexyl", &["hexyl"]),
    ("yq", &["go-yq"]),
    ("kolourpaint", &["kolourpaint"]),
    ("podman", &["podman", "podman-compose"]),
    ("pulumi", &["pulumi"]),
    ("reflector", &["reflector"]),
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
    ("systemd", &["systemd-lsp"]),
    ("wasm", &["wasm-pack"]),
    ("glab", &["glab"]),
    (
        "sway",
        &[
            "sway",
            "swaybg",
            "swaylock",
            "swayidle",
            "xorg-xwayland",
            "xorg-xlsclients",
        ],
    ),
    ("i3status-rust", &["i3status-rust"]),
    ("mousepad", &["mousepad"]),
    ("graphviz", &["graphviz", "xdot"]),
    ("wireguard", &["wireguard-tools"]),
    ("sccache", &["sccache"]),
    // build dependencies for included packages
    ("pkgbuilds-nm-sidebar", &["meson", "ninja"]),
]);

fn hera<'elevate: 'action, 'action, 'secrets>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, 'secrets, Secrets>,
    elevate: &'elevate Elevate,
) {
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
        elevate,
        Packages {
            install: Ref(packages),
            reason: InstallReason::Explicit,
        },
    );

    plan.register_packages(packages);

    plan.add(
        Ref("create user"),
        elevate,
        User {
            name: "herta",
            home: Ref(Path::new("/home/herta")),
            primary_group: PrimaryGroupConfig::Automatic,
            additional_groups: &["wheel", "sudonopw", "games", "kvm", "video"],
            shell: "/usr/bin/zsh",
            system: false,
        },
    );

    plan.add(
        Ref("enable plasma-login-manager"),
        elevate,
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
        elevate,
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
        elevate,
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
}

fn autoupdate<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
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
                    WantedBy=multi-user.target
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
}

fn restic_backup<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
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
                            WantedBy=multi-user.target
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
}

fn initrd<'name, 'elevate: 'action, 'action>(
    plan: &mut Plan<'name, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
    config: &Config,
) {
    plan.with(elevate, |mut plan| {
        let lz4 = &[Ref("lz4")];
        plan.add(
            "install lz4",
            Packages {
                install: Ref(lz4),
                reason: InstallReason::Explicit,
            },
        );

        plan.register_packages(&Ref(lz4));

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
    for socket in [
        "libvirtd.socket",
        "libvirtd-tls.socket",
        "libvirtd-tcp.socket",
        "libvirtd-ro.socket",
        "libvirtd-admin.socket",
    ] {
        plan.add(
            Owned(format!("mask {socket}")),
            elevate,
            Service {
                name: socket,
                state: ServiceState {
                    runtime: ServiceStateRuntime::Inactive,
                    persistent: ServiceStatePersistent::Masked,
                },
            },
        );
    }

    // the libvirtd unit file contains the following setting:
    //
    // Environment=LIBVIRTD_ARGS="--timeout 120"
    //
    // This will make libvirtd stop after 120 seconds without connections or running
    // domains. To convince the daemon to just keep being a daemon, this needs to be
    // removed. Fortunately, the unit also contains the following:
    //
    // EnvironmentFile=-/etc/conf.d/libvirtd
    //
    // And `EnvironmentFile` trumps `Environment`. Otherwise we'd need to do some
    // damn systemd override shenanigans.
    plan.add(
        "configure libvirtd env variable override",
        elevate,
        File {
            path: Ref(Path::new("/etc/conf.d/libvirtd")),
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
            content: Lines(Ref(&[Ref("LIBVIRTD_ARGS=")])),
        },
    )
    .on_change(|| {
        Resource::new(
            Ref("restart libvirt"),
            elevate,
            RestartService {
                name: "libvirtd.service",
            },
        )
    });

    plan.add(
        Ref("enable libvirt"),
        elevate,
        Service {
            name: "libvirtd.service",
            state: ServiceState {
                runtime: ServiceStateRuntime::Active,
                persistent: ServiceStatePersistent::Enabled,
            },
        },
    );
}

fn services<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    for service in ["NetworkManager.service", "systemd-timesyncd.service"] {
        plan.add(
            Owned(format!("enable service {service}")),
            elevate,
            Service {
                name: service,
                state: ServiceState {
                    runtime: ServiceStateRuntime::Active,
                    persistent: ServiceStatePersistent::Enabled,
                },
            },
        );
    }

    #[expect(
        clippy::single_element_loop,
        reason = "consistency with enabled services"
    )]
    for socket in ["pcscd.socket"] {
        plan.add(
            Owned(format!("enable socket {socket}")),
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
}

fn aur<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
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
        let package_path = included_packages_directory.join(filename);

        plan.add(
            Owned(format!("deploy included package {name}")),
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
            Owned(format!("install included package {name}")),
            elevate,
            LocalPackage {
                name: Ref(name),
                path: Owned(package_path),
                version: Ref(version),
                reason: InstallReason::Explicit,
            },
        );

        plan.register_packages(&[Ref(name)]);
    }
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
}

fn timesyncd<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
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
}

fn reflector<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
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
}

fn wireguard<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) -> Result<()> {
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

    Ok(())
}

fn sshd<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) -> Result<()> {
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
}

fn iptables<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
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
}

fn pacman<'plan, 'elevate: 'action, 'action>(
    plan: &'plan mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
    plan.with(elevate, |mut plan| {
        plan.add(
            "configure pacman",
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
    });
}

fn pacman_cache_cleanup<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
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
                Ref("WantedBy=multi-user.target"),
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
}

fn install_packages<'elevate: 'action, 'action, 'pkgs>(
    plan: &mut Plan<'_, 'elevate, 'pkgs, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
    packages: &'pkgs PackageConfig,
) {
    for &(section, packages) in packages.0 {
        let packages = packages
            .iter()
            .map(|package| Ref(*package))
            .collect::<Vec<_>>();

        plan.add(
            Owned(format!("install packages for {section}")),
            elevate,
            Packages {
                install: Owned(packages.clone()),
                reason: InstallReason::Explicit,
            },
        );

        plan.register_packages(&packages);
    }
}

fn hardware<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
    config: &Config,
) {
    match config.machine.gpu {
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
    }

    match config.machine.cpu {
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
    }
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

fn kernel_modules<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) {
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
                }
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
}

fn root_setup<'elevate: 'action, 'action, 'pkgs>(
    plan: &mut Plan<'_, 'elevate, 'pkgs, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
    config: &Config,
    packages: &'pkgs PackageConfig,
) -> Result<()> {
    pacman(plan, elevate);
    install_packages(plan, elevate, packages);
    aur(plan, elevate);
    pacman_cache_cleanup(plan, elevate);
    reflector(plan, elevate);
    initrd(plan, elevate, config);
    libvirt(plan, elevate);
    services(plan, elevate);
    sshd(plan, elevate)?;
    boot_target(plan, elevate, config);
    timesyncd(plan, elevate);
    sudo(plan, elevate);
    hardware(plan, elevate, config);
    logind(plan, elevate);
    journald(plan, elevate);
    wireguard(plan, elevate)?;
    iptables(plan, elevate);
    kernel_modules(plan, elevate);
    backlight(plan, elevate);
    firefox(plan, elevate);

    Ok(())
}

async fn configure_user<'elevate: 'action, 'action, 'user: 'action, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    root: &'elevate Elevate,
    user: &'elevate UserContext,
    machine: &'machine MachineConfig,
) -> Result<()> {
    let UserContext {
        simplest_elevate: ref user_elevate,
        ref strict_elevate,
        config: ref user,
    } = *user;

    let home = PathBuf::from("/home/").join(&user.name);

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

        UserDirectories {
            config,
            bin: home.join("bin"),
            scripts: home.join("scripts"),
            tmp: home.join("tmp"),
            download: home.join("download"),
            systemd_user_units: systemd.join("user"),
            data: local.join("share"),
            systemd,
            local,
        }
    };

    user_create_directories(plan, user_elevate, user, &user_directories);
    Box::pin(user_executables(
        plan,
        user_elevate,
        user,
        &user_directories,
    ))
    .await?;
    user_dotfiles(plan, user_elevate, user, machine, &home)?;
    user_wallpaper(plan, user_elevate, user, &user_directories);
    user_services(plan, user_elevate, user, &user_directories, machine);
    user_spotify(plan, user_elevate, user, &user_directories);
    user_firefox(plan, user_elevate, user, &home);

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

    Ok(())
}

fn user_firefox<'elevate: 'action, 'action, 'user, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    user_elevate: &'elevate Elevate,
    user: &'user UserConfig,
    home: &Path,
) {
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
}

fn user_create_directories<'elevate: 'action, 'action, 'user, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    user_elevate: &'elevate Elevate,
    user: &'elevate UserConfig,
    user_directories: &UserDirectories,
) {
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
}

fn user_spotify<'elevate: 'action, 'action, 'user, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    user_elevate: &'elevate Elevate,
    user: &'user UserConfig,
    user_directories: &UserDirectories,
) {
    plan.add(
        Owned("fix spotify audio api".to_owned()),
        user_elevate,
        File {
            path: Owned(user_directories.config.join("spotify-flags.conf")),
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
            content: FileContent::Lines(Ref(&[Ref("--audio-api=pulseaudio")])),
        },
    );
}

fn user_autologin<'elevate: 'action, 'action, 'user, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    root: &'elevate Elevate,
    user: &'user UserConfig,
) {
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
}

fn user_dotfiles<'elevate: 'action, 'action, 'user, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    user_elevate: &'elevate Elevate,
    user: &'user UserConfig,
    machine: &MachineConfig,
    home: &Path,
) -> Result<()> {
    #[derive(Serialize)]
    struct TemplateContext<'a> {
        user: &'a UserConfig,
        machine: &'a MachineConfig,
        is_laptop: bool,
        is_workstation: bool,
        is_tv: bool,
    }

    for (directory, mode) in [
        (".config/rofi", None),
        (".config/gtk-3.0", None),
        (".config/qt5ct", None),
        (".config/i3status-rust", None),
        (".config/i3status-rust/icons", None),
        (".config/i3status-rust/themes", None),
        (".config/helix", None),
        (".config/git", None),
        (".config/tmux", None),
        (".config/sway", None),
        (".config/sway/scripts", None),
        (".config/zsh", None),
        (".config/alacritty", None),
        (".local/state/", None),
        (".local/state/gnupg/", Some(FileMode(0o700))),
        (".local/state/cargo/", None),
        (".local/share/applications", None),
        (".config/VSCodium/", None),
        (".config/VSCodium/User", None),
        (".config/alacritty/themes/", None),
    ] {
        plan.add(
            Owned(format!("create empty directory {directory}")),
            user_elevate,
            Directory {
                path: Owned(home.join(directory)),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: mode.unwrap_or(FileMode(0o755)),
                },
            },
        );
    }

    for to_remove in [
        ".gitconfig",
        ".vimrc",
        ".config/nvim/init.vim",
        ".tmux.conf",
        ".i3",
        ".gtkrc-2.0",
        ".opt",
        ".optbin",
        ".usr",
        ".var",
        ".screenrc",
        ".zshrc.d",
    ] {
        plan.add(
            Owned(format!("remove entry {to_remove}")),
            user_elevate,
            Remove {
                path: Owned(home.join(to_remove)),
                on_directory: DirectoryHandling::Proceed,
                on_file: FileHandling::Proceed,
                on_symlink: SymlinkHandling::Proceed,
            },
        );
    }

    for (path, content) in [
        (
            ".local/share/applications/firefox.desktop",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/desktop/firefox.desktop"
            ))
            .as_slice(),
        ),
        (
            ".config/alacritty/themes/monokai.toml",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/alacritty/themes/monokai.toml"
            ))
            .as_slice(),
        ),
        (
            ".config/alacritty/themes/github_light.toml",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/alacritty/themes/github_light.toml"
            ))
            .as_slice(),
        ),
        (
            ".local/state/cargo/config.toml",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/cargo/config.toml"
            ))
            .as_slice(),
        ),
        (
            ".local/state/gnupg/dirmngr.conf",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/gnupg/dirmngr.conf"
            ))
            .as_slice(),
        ),
        (
            ".local/state/gnupg/scdaemon.conf",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/gnupg/scdaemon.conf"
            ))
            .as_slice(),
        ),
        (
            ".config/i3status-rust/themes/native.toml",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/i3status-rust/themes/native.toml"
            ))
            .as_slice(),
        ),
        (
            ".config/i3status-rust/icons/awesome.toml",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/i3status-rust/icons/awesome.toml"
            ))
            .as_slice(),
        ),
        (
            ".config/tmux/tmux.conf",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/tmux/tmux.conf"
            ))
            .as_slice(),
        ),
        (
            ".zshenv",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../files/zsh/zshenv")).as_slice(),
        ),
        (
            ".config/gammastep.conf",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/gammastep/gammastep.conf"
            ))
            .as_slice(),
        ),
        (
            ".config/VSCodium/product.json",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/vscodium/product.json"
            ))
            .as_slice(),
        ),
        (
            ".config/VSCodium/User/settings.json",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/vscodium/settings.json"
            ))
            .as_slice(),
        ),
        (
            ".config/VSCodium/User/keybindings.json",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/vscodium/keybindings.json"
            ))
            .as_slice(),
        ),
        (
            ".config/rofi/config",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../files/rofi/config")).as_slice(),
        ),
        (
            ".config/qt5ct/qt5ct.conf",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/qt/qt5ct.conf"
            ))
            .as_slice(),
        ),
        (
            ".config/helix/config.toml",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/helix/config.toml"
            ))
            .as_slice(),
        ),
        (
            ".config/helix/languages.toml",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/helix/languages.toml"
            ))
            .as_slice(),
        ),
        (
            ".local/state/cargo/config.toml",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/cargo/config.toml"
            ))
            .as_slice(),
        ),
        (
            ".config/git/ignore",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../files/git/ignore")).as_slice(),
        ),
        (
            ".config/sway/scripts/swap-from-workspace.py",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/sway/scripts/swap-from-workspace.py"
            ))
            .as_slice(),
        ),
    ] {
        plan.add(
            Owned(format!("{user}: add dotfile ~/{path}", user = user.name)),
            user_elevate,
            File {
                path: Owned(home.join(path)),
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

    let template_context = TemplateContext {
        user,
        machine,
        is_laptop: machine.machine_type == Laptop,
        is_workstation: machine.machine_type == Workstation,
        is_tv: machine.machine_type == Tv,
    };

    let template_context = tera::Context::from_serialize(&template_context)?;

    for (path, template) in [
        (
            ".config/git/config",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/git/gitconfig.j2"
            )),
        ),
        (
            ".local/state/gnupg/gpg-agent.conf",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/gnupg/gpg-agent.conf.j2"
            )),
        ),
        (
            ".local/state/gnupg/gpg.conf",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/gnupg/gpg.conf.j2"
            )),
        ),
        (
            ".config/zsh/.zprofile",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/zsh/zprofile.j2"
            )),
        ),
        (
            ".config/zsh/.zshrc",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/zsh/zshrc.j2"
            )),
        ),
        (
            ".config/dunstrc",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/dunst/dunstrc.j2"
            )),
        ),
        (
            ".config/alacritty/config.toml",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/alacritty/alacritty.toml.j2"
            )),
        ),
        (
            ".config/gtk-3.0/settings.ini",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/gtk/gtk-3.0.ini.j2"
            )),
        ),
        (
            ".config/gtkrc-2.0",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/gtk/gtkrc-2.0.j2"
            )),
        ),
        (
            ".config/i3status-rust/config.toml",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/i3status-rust/config.toml.j2"
            )),
        ),
        (
            ".config/sway/config",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/sway/config.j2"
            )),
        ),
    ] {
        let template = tera::Tera::one_off(template, &template_context, false)?;
        plan.add(
            Owned(format!("{user}: add dotfile ~/{path}", user = user.name)),
            user_elevate,
            File {
                path: Owned(home.join(path)),
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
                content: FileContent::Bytes(Owned(template.into_bytes())),
            },
        );
    }

    Ok(())
}

async fn user_executables<'elevate: 'action, 'action, 'user, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    user_elevate: &'elevate Elevate,
    user: &'user UserConfig,
    user_directories: &UserDirectories,
) -> Result<()> {
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
                let path =
                    Path::from_path(file.path()).ok_or_else(|| anyhow!("non utf-8 path in bin"))?;

                let name = path.file_name().expect("file to have a name");

                Ok((name, file.contents()))
            }
        })
        .collect::<Result<_, _>>()?;

    for &(name, content) in &bins {
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

    Ok(())
}

fn user_wallpaper<'elevate: 'action, 'action, 'user, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
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
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    user_elevate: &'elevate Elevate,
    user: &'user UserConfig,
    user_directories: &UserDirectories,
    machine: &MachineConfig,
) {
    for (unit, content, enabled) in [
        (
            "dunst.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/autostart/services/dunst.service"
            ))
            .as_slice(),
            true,
        ),
        (
            "firefox@.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/autostart/services/firefox@.service"
            ))
            .as_slice(),
            true,
        ),
        (
            "gammastep.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/autostart/services/gammastep.service"
            ))
            .as_slice(),
            machine.machine_type != MachineType::Tv,
        ),
        (
            "gnome-keyring.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/autostart/services/gnome-keyring.service"
            ))
            .as_slice(),
            true,
        ),
        (
            "gpg-agent.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/autostart/services/gpg-agent.service"
            ))
            .as_slice(),
            user.gpg.as_ref().is_some_and(|gpg| gpg.agent_enabled),
        ),
        (
            "kdeconnect.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/autostart/services/kdeconnect.service"
            ))
            .as_slice(),
            true,
        ),
        (
            "keepassxc.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/autostart/services/keepassxc.service"
            ))
            .as_slice(),
            user.keepassxc.enabled(),
        ),
        (
            "laptop-lid.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/autostart/services/laptop-lid.service"
            ))
            .as_slice(),
            machine.machine_type == Laptop,
        ),
        (
            "nextcloud.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/autostart/services/nextcloud.service"
            ))
            .as_slice(),
            user.nextcloud.enabled(),
        ),
        (
            "nm-sidebar.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/autostart/services/nm-sidebar.service"
            ))
            .as_slice(),
            true,
        ),
        (
            "screencfg.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/autostart/services/screencfg.service"
            ))
            .as_slice(),
            true,
        ),
        (
            "spotify.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/autostart/services/spotify.service"
            ))
            .as_slice(),
            true,
        ),
        (
            "steam.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/autostart/services/steam.service"
            ))
            .as_slice(),
            user.steam.enabled(),
        ),
        (
            "sway.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/autostart/services/sway.service"
            ))
            .as_slice(),
            true,
        ),
        (
            "windowmanager.target",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/autostart/services/windowmanager.target"
            ))
            .as_slice(),
            true,
        ),
        (
            "workstation-mgr.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/autostart/services/workstation-mgr.service"
            ))
            .as_slice(),
            true,
        ),
        (
            "yubikey-touch-detector.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/autostart/services/yubikey-touch-detector.service"
            ))
            .as_slice(),
            user.gpg.as_ref().is_some_and(|gpg| gpg.agent_enabled),
        ),
    ] {
        let path = Owned(user_directories.systemd_user_units.join(unit));

        if enabled {
            plan.add(
                Owned(format!("user autostart service {unit}")),
                user_elevate,
                File {
                    path,
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
        } else {
            plan.add(
                Owned(format!("user service {unit}")),
                user_elevate,
                Remove {
                    path,
                    on_directory: DirectoryHandling::Bail,
                    on_file: FileHandling::Proceed,
                    on_symlink: SymlinkHandling::Proceed,
                },
            );
        }
    }

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
}

#[derive(Eq, PartialEq, Serialize)]
enum MachineType {
    Tv,
    Workstation,
    Laptop,
}
use MachineType::{Laptop, Tv, Workstation};

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

#[derive(Serialize)]
struct MachineConfig {
    encrypted_root: bool,
    boot_target: Target,
    cpu: Cpu,
    gpu: Gpu,
    font_size: usize,
    workspace: WorkspaceConfig,
    machine_type: MachineType,
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

async fn orphans<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action, '_, Secrets>,
    elevate: &'elevate Elevate,
) -> Result<()> {
    let orphans = list_orphaned_packages(elevate).await?;

    plan.with(elevate, |mut plan| {
        for orphan in orphans {
            plan.add(
                Owned(format!("remove orphaned package {orphan}")),
                PackageRemove {
                    remove: Owned(vec![Owned(orphan)]),
                },
            );
        }
    });

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
        .collect::<Vec<String>>();

    let explicitly_installed_packages = list_explicitly_installed_packages(elevate).await?;

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
                    },
                    Ares => MachineConfig {
                        encrypted_root: true,
                        boot_target: Target::MultiUser,
                        cpu: Cpu::Amd,
                        gpu: Gpu::Amd,
                        font_size: 10,
                        workspace: WorkspaceConfig {
                            num1: Some("".to_owned()),
                            num2: None,
                            num3: None,
                            num4: None,
                            num5: None,
                            num6: None,
                            num7: None,
                            num8: None,
                            num9: Some("".to_owned()),
                            num0: Some("".to_owned()),
                        },
                        machine_type: Workstation,
                    },
                    Hera => MachineConfig {
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
                            num7: Some("".to_owned()),
                            num8: Some("".to_owned()),
                            num9: Some("".to_owned()),
                            num0: Some("".to_owned()),
                        },
                        machine_type: Laptop,
                    },
                    Dionysus => MachineConfig {
                        encrypted_root: true,
                        boot_target: Target::MultiUser,
                        cpu: Cpu::Intel,
                        gpu: Gpu::Intel,
                        font_size: 11,
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

                root_setup(&mut plan, &root, &config, &PACKAGES)?;

                for user in &config.users {
                    configure_user(&mut plan, &root, user, &config.machine).await?;
                }

                match args.hostname {
                    Hera => {
                        hera(&mut plan, &root);
                        autoupdate(&mut plan, &root);
                        restic_backup(&mut plan, &root);
                    }
                    Dionysus => {
                        autoupdate(&mut plan, &root);
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
                            FinishResult::AssertionsFailed { report } => {
                                unreachable!("removal plan does not contain assertions")
                            }
                            FinishResult::Success(plan) => plan,
                        };

                        let cleanup_report = cleanup_plan_finished.report()?;

                        // it's only package remove resources anyway, will always be changed
                        cleanup_report.print(&ReportConfig::All, false);

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
                            FinishResult::AssertionsFailed { report } => {
                                unreachable!("removal plan does not contain assertions")
                            }
                            FinishResult::Success(plan) => plan,
                        };

                        let cleanup_report = cleanup_plan_finished.report()?;

                        // it's only package remove resources anyway, will always be changed
                        cleanup_report.print(&ReportConfig::All, false);

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
