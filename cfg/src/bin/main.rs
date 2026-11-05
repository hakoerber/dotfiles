#![expect(
    clippy::large_include_file,
    reason = "Everything necessary should be included in the binary (PKGBUILDS, locally built packages)"
)]
#![allow(unused, reason = "dev")]
// required for `serde_json::json!()`
#![recursion_limit = "256"]

use std::{collections::HashMap, env, fs};

use anyhow::{Result, anyhow, bail};
use clap::{Parser, ValueEnum};
use include_dir::include_dir;
use regex::Regex;
use serde::{Deserialize, Serialize};

use cfg::prelude::*;

static PKGBUILDS: include_dir::Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../pkgbuilds");

static INCLUDED_PACKAGES: &[(&str, &str, &str, &[u8])] =
    include!(concat!(env!("OUT_DIR"), "/packages.rs"));

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

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    dry_run: bool,

    #[arg(long)]
    debug: bool,

    #[arg(long)]
    only_changed: bool,

    #[arg(long)]
    hostname: Host,
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
    ("apt", &["apt"]),
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
    ("feh", &["feh"]),
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
            "iptables",
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
    ("iptables", &["iptables"]),
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
    (
        "docker",
        &[
            "docker",
            "docker-compose",
            "docker-buildx",
            "dockerfile-language-server",
        ],
    ),
    ("xdg", &["xdg-utils"]),
    ("dunst", &["dunst"]),
    ("cloc", &["cloc"]),
    ("bwm-ng", &["bwm-ng"]),
    ("ssh", &["openssh"]),
    ("sshfs", &["sshfs"]),
    ("expect", &["expect"]),
    ("inotify", &["inotify-tools"]),
    ("rclone", &["rclone"]),
    ("dnf", &["dnf5"]),
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
    ("element", &["element-desktop"]),
    ("mkinitcpio", &["mkinitcpio"]),
    ("terraform", &["terraform", "terragrunt"]),
    ("ncdu", &["ncdu"]),
    ("dust", &["dust"]),
    ("font-utils", &["woff2"]),
    ("jq", &["jq"]),
    ("musl", &["musl"]),
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
    ("zeal", &["zeal"]),
    ("kcharselect", &["kcharselect"]),
    ("bottom", &["bottom"]),
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
    ("drone", &["drone-cli"]),
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
    ("picom", &["picom"]),
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
]);

fn hera<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
    config: &Config,
) -> Result<()> {
    let packages = &[
        Ref("plasma-desktop"),
        Ref("konsole"),
        Ref("dolphin"),
        Ref("kdeplasma-addons"),
        Ref("plasma-nm"),
        Ref("plasma-pa"),
        Ref("plasma-systemmonitor"),
        Ref("sddm"),
        Ref("sddm-kcm"),
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
        Box::new(Packages {
            install: Ref(packages),
            reason: InstallReason::Explicit,
        }),
    )?;

    plan.register_packages(&Ref(packages));

    plan.add(
        Ref("create user"),
        elevate,
        Box::new(User {
            name: "herta",
            home: Ref(Path::new("/home/herta")),
            primary_group: PrimaryGroupConfig::Automatic,
            additional_groups: &["wheel", "sudonopw", "games", "kvm", "video"],
            shell: "/usr/bin/zsh",
            system: false,
        }),
    )?;

    plan.add(
        Ref("enable sddm"),
        elevate,
        Box::new(Service {
            name: "sddm.service",
            state: ServiceState {
                runtime: ServiceStateRuntime::Active,
                persistent: ServiceStatePersistent::Enabled,
            },
        }),
    )?;

    plan.add(
        Ref("create sddm config folder"),
        elevate,
        Box::new(Directory {
            path: Ref(Path::new("/etc/sddm.conf.d/")),
            on_symlink: SymlinkHandling::Proceed,
            on_file: FileHandling::Proceed,
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o755),
            },
        }),
    )?;

    plan.add(
        "enable autologin",
        elevate,
        Box::new(File {
            path: Ref(Path::new("/etc/sddm.conf.d/autologin.conf")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o644),
            },
            on_symlink: SymlinkHandling::Proceed,
            on_directory: DirectoryHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: FileContent::Bytes(Owned(
                textwrap::dedent(
                    r#"
                    [Autologin]
                    User=herta
                    Session=plasma
                    "#,
                )
                .into_bytes(),
            )),
        }),
    )?;

    plan.add(
        "lock on startup",
        elevate,
        Box::new(File {
            path: Ref(Path::new("/etc/xdg/kscreenlockerrc")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o644),
            },
            on_symlink: SymlinkHandling::Proceed,
            on_directory: DirectoryHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: FileContent::Bytes(Owned(
                textwrap::dedent(
                    r#"
                    [Daemon]
                    LockOnStart=true
                    "#,
                )
                .into_bytes(),
            )),
        }),
    )?;

    Ok(())
}

fn orphans<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
    config: &Config,
) -> Result<()> {
    let orphans = list_orphaned_packages(elevate)?;

    for orphan in orphans {
        plan.add(
            Owned(format!("remove orphaned package {orphan}")),
            elevate,
            Box::new(PackageRemove {
                remove: Owned(vec![Owned(orphan)]),
            }),
        )?;
    }

    Ok(())
}

fn unmanaged_packages<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
    config: &Config,
) -> Result<()> {
    let managed_packages = plan.managed_packages();

    let protected = [
        "base", // virtual package, would not hurt, but it's weird if its not installed
        "workstation-mgr",
    ];

    let managed_packages = managed_packages
        .iter()
        .map(|s| s.to_owned())
        .collect::<Vec<String>>();

    let explicitly_installed_packages = list_explicitly_installed_packages(elevate)?;

    for installed_package in explicitly_installed_packages {
        if !managed_packages.contains(&installed_package)
            && !protected.contains(&installed_package.as_str())
        {
            plan.add(
                Owned(format!("remove unmanaged package {installed_package}")),
                elevate,
                Box::new(PackageRemove {
                    remove: Owned(vec![Owned(installed_package)]),
                }),
            )?;
        }
    }

    Ok(())
}

fn autoupdate<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
    config: &Config,
) -> Result<()> {
    plan.add(
        "deploy autoupdate script",
        elevate,
        Box::new(File {
            path: Ref(Path::new("/usr/local/bin/pacman-autoupdate")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o755),
            },
            on_symlink: SymlinkHandling::Proceed,
            on_directory: DirectoryHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: FileContent::Bytes(Owned(
                textwrap::dedent(
                    r#"
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

                    pacman --noprogressbar --noconfirm --sysupgrade
                    "#,
                )
                .into_bytes(),
            )),
        }),
    )?;

    plan.add(
        "install pacman autoupdate service",
        elevate,
        Box::new(File {
            path: Ref(Path::new("/etc/systemd/system/pacman-autoupdate.service")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o644),
            },
            on_symlink: SymlinkHandling::Proceed,
            on_directory: DirectoryHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: FileContent::Bytes(Owned(
                textwrap::dedent(
                    r#"
                        [Service]
                        Type=oneshot
                        ExecStart=/usr/local/bin/pacman-autoupdate
                        "#,
                )
                .into_bytes(),
            )),
        }),
    )?
    .on_change(Box::new(|| {
        Ok(Resource::new(
            Ref("restart service manager due to new packages"),
            elevate,
            Box::new(DaemonReload {}),
        ))
    }))?;

    plan.add(
        "install pacman autoupdate timer",
        elevate,
        Box::new(File {
            path: Ref(Path::new("/etc/systemd/system/pacman-autoupdate.timer")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o644),
            },
            on_symlink: SymlinkHandling::Proceed,
            on_directory: DirectoryHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: FileContent::Bytes(Owned(
                textwrap::dedent(
                    r#"
                        [Timer]
                        OnCalendar=daily
                        OnBootSec=5min
                        OnUnitInactiveSec=120min

                        [Install]
                        WantedBy=multi-user.target
                        "#,
                )
                .into_bytes(),
            )),
        }),
    )?
    .on_change(Box::new(|| {
        Ok(Resource::new(
            Ref("restart service manager due to new packages"),
            elevate,
            Box::new(DaemonReload {}),
        ))
    }))?;

    plan.add(
        Ref("enable pacman autoupdate timer"),
        elevate,
        Box::new(Service {
            name: "pacman-autoupdate.timer",
            state: ServiceState {
                runtime: ServiceStateRuntime::Active,
                persistent: ServiceStatePersistent::Enabled,
            },
        }),
    )?;

    Ok(())
}

fn restic_backup<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
    config: &Config,
) -> Result<()> {
    plan.add(
        Ref("create restic config directory"),
        elevate,
        Box::new(Directory {
            path: Ref(Path::new("/etc/restic")),
            on_symlink: SymlinkHandling::Proceed,
            on_file: FileHandling::Proceed,
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o755),
            },
        }),
    )?;

    plan.add(
        Ref("create restic cache directory"),
        elevate,
        Box::new(Directory {
            path: Ref(Path::new("/var/cache/restic")),
            on_symlink: SymlinkHandling::Proceed,
            on_file: FileHandling::Proceed,
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o755),
            },
        }),
    )?;

    plan.add(
        "create restic exclude file",
        elevate,
        Box::new(File {
            path: Ref(Path::new("/etc/restic/exclude.lst")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o644),
            },
            on_symlink: SymlinkHandling::Proceed,
            on_directory: DirectoryHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: FileContent::Bytes(Owned(
                textwrap::dedent(
                    r#"
                    /home/*/.cache/**
                    /home/*/.mozilla/firefox/*/Cache/**
                    "#,
                )
                .into_bytes(),
            )),
        }),
    )?;

    plan.add(
        "create restic wrapper script",
        elevate,
        Box::new(File {
            path: Ref(Path::new("/usr/local/bin/restic-cmd")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o755),
            },
            on_symlink: SymlinkHandling::Proceed,
            on_directory: DirectoryHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: FileContent::Bytes(Owned(
                textwrap::dedent(
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
                    "#,
                )
                .into_bytes(),
            )),
        }),
    )?;

    plan.add(
        "create restic backup script",
        elevate,
        Box::new(File {
            path: Ref(Path::new("/usr/local/bin/restic-backup")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o755),
            },
            on_symlink: SymlinkHandling::Proceed,
            on_directory: DirectoryHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: FileContent::Bytes(Owned(
                textwrap::dedent(
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
                    "#,
                )
                .into_bytes(),
            )),
        }),
    )?;

    plan.add(
        "install restic backup service",
        elevate,
        Box::new(File {
            path: Ref(Path::new("/etc/systemd/system/restic-backup.service")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o644),
            },
            on_symlink: SymlinkHandling::Proceed,
            on_directory: DirectoryHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: FileContent::Bytes(Owned(
                textwrap::dedent(
                    r#"
                        [Service]
                        Type=oneshot
                        ExecStart=systemd-inhibit /usr/local/bin/restic-backup
                        "#,
                )
                .into_bytes(),
            )),
        }),
    )?
    .on_change(Box::new(|| {
        Ok(Resource::new(
            Ref("restart service manager due to new packages"),
            elevate,
            Box::new(DaemonReload {}),
        ))
    }));

    plan.add(
        "install restic backup timer",
        elevate,
        Box::new(File {
            path: Ref(Path::new("/etc/systemd/system/restic-backup.timer")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o644),
            },
            on_symlink: SymlinkHandling::Proceed,
            on_directory: DirectoryHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: FileContent::Bytes(Owned(
                textwrap::dedent(
                    r#"
                        [Timer]
                        OnCalendar=daily
                        Persistent=true

                        [Install]
                        WantedBy=multi-user.target
                        "#,
                )
                .into_bytes(),
            )),
        }),
    )?
    .on_change(Box::new(|| {
        Ok(Resource::new(
            Ref("restart service manager due to new packages"),
            elevate,
            Box::new(DaemonReload {}),
        ))
    }));

    plan.add(
        Ref("enable pacman autoupdate timer"),
        elevate,
        Box::new(Service {
            name: "restic-backup.timer",
            state: ServiceState {
                runtime: ServiceStateRuntime::Active,
                persistent: ServiceStatePersistent::Enabled,
            },
        }),
    )?;

    Ok(())
}

fn initrd<'name, 'elevate: 'action, 'action>(
    plan: &mut Plan<'name, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
    config: &Config,
) -> Result<()> {
    let lz4 = &[Ref("lz4")];
    plan.add(
        "install lz4",
        elevate,
        Box::new(Packages {
            install: Ref(lz4),
            reason: InstallReason::Explicit,
        }),
    )?;

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

    let rebuild_mkinitcpio = Box::new(Command {
        cmd: Ref("mkinitcpio"),
        args: &["-P"],
        check_command: None,
    });

    plan.add(
        "configure mkinitcpio",
        elevate,
        Box::new(File {
            path: Ref(Path::new("/etc/mkinitcpio.conf")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o644),
            },
            on_symlink: SymlinkHandling::Proceed,
            on_directory: DirectoryHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: FileContent::Bytes(Owned(
                textwrap::dedent(&format!(
                    r#"
                    MODULES=()
                    BINARIES=()
                    FILES=()
                    HOOKS=({hooks})
                    COMPRESSION="lz4"
                    "#,
                    hooks = hooks.join(" "),
                ))
                .into_bytes(),
            )),
        }),
    )?
    .on_change(Box::new(|| {
        Ok(Resource::new(
            Ref("rebuild initrd"),
            elevate,
            rebuild_mkinitcpio,
        ))
    }))?;

    Ok(())
}

fn libvirt<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
) -> Result<()> {
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
            Box::new(Service {
                name: socket,
                state: ServiceState {
                    runtime: ServiceStateRuntime::Inactive,
                    persistent: ServiceStatePersistent::Masked,
                },
            }),
        )?;
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
        Box::new(File {
            path: Ref(Path::new("/etc/conf.d/libvirtd")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o600),
            },
            on_directory: DirectoryHandling::Bail,
            on_symlink: SymlinkHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: Lines(Ref(&[Ref("LIBVIRTD_ARGS=")])),
        }),
    )?
    .on_change(Box::new(|| {
        Ok(Resource::new(
            Ref("restart libvirt"),
            elevate,
            Box::new(RestartService {
                name: "libvirtd.service",
            }),
        ))
    }))?;

    plan.add(
        Ref("enable libvirt"),
        elevate,
        Box::new(Service {
            name: "libvirtd.service",
            state: ServiceState {
                runtime: ServiceStateRuntime::Active,
                persistent: ServiceStatePersistent::Enabled,
            },
        }),
    )?;

    Ok(())
}

fn services<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
) -> Result<()> {
    #[expect(
        clippy::single_element_loop,
        reason = "consistency with enabled services"
    )]
    for service in ["sshd.service"] {
        plan.add(
            Owned(format!("disable service {service}")),
            elevate,
            Box::new(Service {
                name: service,
                state: ServiceState {
                    runtime: ServiceStateRuntime::Inactive,
                    persistent: ServiceStatePersistent::Disabled,
                },
            }),
        )?;
    }

    for service in [
        "NetworkManager.service",
        "docker.service",
        "systemd-timesyncd.service",
    ] {
        plan.add(
            Owned(format!("enable service {service}")),
            elevate,
            Box::new(Service {
                name: service,
                state: ServiceState {
                    runtime: ServiceStateRuntime::Active,
                    persistent: ServiceStatePersistent::Enabled,
                },
            }),
        )?;
    }

    #[expect(
        clippy::single_element_loop,
        reason = "consistency with enabled services"
    )]
    for socket in ["pcscd.socket"] {
        plan.add(
            Owned(format!("enable socket {socket}")),
            elevate,
            Box::new(Service {
                name: socket,
                state: ServiceState {
                    runtime: ServiceStateRuntime::Active,
                    persistent: ServiceStatePersistent::Enabled,
                },
            }),
        );
    }

    Ok(())
}

fn copy_dir<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
    source: &include_dir::Dir,
    root: &Path,
    target: &Path,
    makepkg_user: &Uid,
    makepkg_group: &Gid,
) -> Result<()> {
    let source_path =
        Path::from_path(source.path()).ok_or_else(|| anyhow!("non utf-8 path in pkgbuiilds"))?;

    plan.add(
        Owned(format!("create pkgbuild folder for {target}")),
        elevate,
        Box::new(Directory {
            path: Owned(target.to_owned()),
            on_symlink: SymlinkHandling::Proceed,
            on_file: FileHandling::Proceed,
            permissions: FilePermissions {
                owner: makepkg_user.clone(),
                group: makepkg_group.clone(),
                mode: FileMode(0o700),
            },
        }),
    );

    for file in source.files() {
        if file
            .path()
            .file_name()
            .expect("file has a filename")
            .to_string_lossy()
            .starts_with('.')
        {
            continue;
        }

        let path = root.join(
            Path::from_path(file.path()).ok_or_else(|| anyhow!("non utf-8 path in pkgbuiilds"))?,
        );

        plan.add(
            Owned(format!("copy pkgbuild file {path}")),
            elevate,
            Box::new(File {
                path: Owned(target.join(path)),
                permissions: FilePermissions {
                    owner: makepkg_user.clone(),
                    group: makepkg_group.clone(),
                    mode: FileMode(0o600),
                },
                on_directory: DirectoryHandling::Bail,
                on_symlink: SymlinkHandling::Proceed,
                binary: true,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Owned(file.contents().to_vec())),
            }),
        );
    }

    for dir in source.dirs() {
        let path = root.join(
            Path::from_path(dir.path()).ok_or_else(|| anyhow!("non utf-8 path in pkgbuiilds"))?,
        );
        copy_dir(plan, elevate, dir, root, &path, makepkg_user, makepkg_group)?;
    }

    Ok(())
}

#[derive(Debug)]
struct Pkgbuild {
    pkgname: String,
    pkgver: String,
    pkgrel: String,
    epoch: Option<String>,
    depends: Vec<String>,
    makedepends: Vec<String>,
    checkdepends: Vec<String>,
}

impl Pkgbuild {
    fn extract_packages(line: &str) -> Vec<String> {
        if line.is_empty() {
            vec![]
        } else {
            line.split(' ').map(|s|
                    // remove all trailing version specifiers
                    match s.split_once(|c|
                        !matches!(c, 'A'..='Z' | 'a'..='z' | '0'..='9' | '@'|'.'|'_'| '+'| '-')
                    ) {
                        Some((name, rest)) => name.to_owned(),
                        None => s.to_owned(),
                    }

                ).collect()
        }
    }

    fn read(elevate: &Elevate, data: &[u8]) -> Result<Self> {
        let source = Process::run(
            "sh",
            &[
                "-c",
                r#"
                    source /dev/stdin ;
                    printf '%s\n' "${pkgname}";
                    printf '%s\n' "${pkgver}";
                    printf '%s\n' "${pkgrel}";
                    printf '%s\n' "${epoch}";
                    printf '%s\n' "${depends[*]}";
                    printf '%s\n' "${makedepends[*]}" ;
                    printf '%s\n' "${checkdepends[*]}"
                "#,
            ],
            elevate,
            Some(data),
        )?
        .check()?;

        let lines = source.stdout.lines().collect::<Vec<_>>();

        let lines: [&str; 7] = lines
            .try_into()
            .map_err(|_err| anyhow!("source returned invalid data"))?;

        Ok(Self {
            pkgname: lines[0].to_owned(),
            pkgver: lines[1].to_owned(),
            pkgrel: lines[2].to_owned(),
            epoch: {
                let line = lines[3];
                (!line.is_empty()).then(|| line.to_owned())
            },
            depends: Self::extract_packages(lines[4]),
            makedepends: Self::extract_packages(lines[5]),
            checkdepends: Self::extract_packages(lines[6]),
        })
    }
}

fn aur<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
) -> Result<()> {
    #[derive(Debug)]
    struct AurPackage {
        name: &'static str,
        preexec: Option<String>,
    }

    let aur_packages = &[
        AurPackage {
            name: "nodejs-intelephense",
            preexec: None,
        },
        AurPackage {
            name: "terraform-ls-bin",
            preexec: None,
        },
        AurPackage {
            name: "grm",
            preexec: None,
        },
        AurPackage {
            name: "screencfg",
            preexec: None,
        },
        AurPackage {
            name: "slack-desktop",
            preexec: None,
        },
        AurPackage {
            name: "claude-code",
            preexec: None,
        },
        AurPackage {
            name: "cargo-lambda-bin",
            preexec: None,
        },
        AurPackage {
            name: "nm-sidebar",
            preexec: None,
        },
        AurPackage {
            name: "aws-session-manager-plugin",
            preexec: None,
        },
        AurPackage {
            name: "portfolio-performance-bin",
            preexec: Some(textwrap::dedent(
                r#"
                source ./env
                curl -sSf --proto '=https' https://keys.openpgp.org/vks/v1/by-fingerprint/E46E6F8FF02E4C83569084589239277F560C95AC | gpg --import -
                "#,
            )),
        },
        AurPackage {
            name: "spotify",
            preexec: Some(textwrap::dedent(
                r#"
                source ./env
                curl -sSf --proto '=https' https://download.spotify.com/debian/pubkey_5384CE82BA52C83A.gpg | gpg --import -
                "#,
            )),
        },
    ];

    let makepkg_user = Uid::from("makepkg".to_owned());
    let makepkg_group = Gid::from("makepkg".to_owned());

    let makepkg_home = Path::new("/var/lib/makepkg");
    let makepkg_build_directory = makepkg_home.join("build");
    let pkgbuild_directory = makepkg_home.join("pkgbuilds");
    let included_packages_directory = makepkg_home.join("packages");

    plan.add(
        Ref("create build user"),
        elevate,
        Box::new(User {
            name: "makepkg",
            home: Ref(makepkg_home),
            primary_group: PrimaryGroupConfig::Automatic,
            additional_groups: &[],
            shell: "/bin/bash",
            system: true,
        }),
    )?;

    plan.add(
        Ref("create included packages directory"),
        elevate,
        Box::new(Directory {
            path: Owned(included_packages_directory.clone()),
            on_symlink: SymlinkHandling::Proceed,
            on_file: FileHandling::Proceed,
            permissions: FilePermissions {
                owner: makepkg_user.clone(),
                group: makepkg_group.clone(),
                mode: FileMode(0o700),
            },
        }),
    )?;

    for &(name, version, filename, content) in INCLUDED_PACKAGES {
        let package_path = included_packages_directory.join(filename);

        plan.add(
            Owned(format!("deploy included package {name}")),
            elevate,
            Box::new(File {
                content: FileContent::Bytes(Ref(content)),
                binary: true,
                // checksum may change, the only thing that matters is the version, which is
                // part of the filename
                ignore_checksum_changes: true,
                path: Owned(package_path.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: makepkg_user.clone(),
                    group: makepkg_group.clone(),
                    mode: FileMode(0o700),
                },
            }),
        )?;

        plan.add(
            Owned(format!("install included package {name}")),
            elevate,
            Box::new(LocalPackage {
                name: Ref(name),
                path: Owned(package_path),
                version: Ref(version),
                reason: InstallReason::Explicit,
            }),
        )?;
    }

    plan.add(
        Ref("create pkgbuild root directory"),
        elevate,
        Box::new(Directory {
            path: Owned(pkgbuild_directory.clone()),
            on_symlink: SymlinkHandling::Proceed,
            on_file: FileHandling::Proceed,
            permissions: FilePermissions {
                owner: makepkg_user.clone(),
                group: makepkg_group.clone(),
                mode: FileMode(0o700),
            },
        }),
    )?;

    plan.add(
        Ref("create build directory"),
        elevate,
        Box::new(Directory {
            path: Owned(makepkg_build_directory.clone()),
            on_symlink: SymlinkHandling::Proceed,
            on_file: FileHandling::Proceed,
            permissions: FilePermissions {
                owner: makepkg_user.clone(),
                group: makepkg_group.clone(),
                mode: FileMode(0o700),
            },
        }),
    );

    PKGBUILDS
        .files()
        .next()
        .is_none()
        .ok_or_else(|| anyhow!("files in root of pkgbuilds"))?;

    for entry in PKGBUILDS.dirs() {
        let entry_path =
            Path::from_path(entry.path()).ok_or_else(|| anyhow!("non utf-8 path in pkgbuiilds"))?;

        copy_dir(
            plan,
            elevate,
            entry,
            &pkgbuild_directory,
            &pkgbuild_directory.join(entry_path),
            &makepkg_user,
            &makepkg_group,
        )?;
    }

    for aur_package in aur_packages {
        let pkgbuild = PKGBUILDS
            .get_file(format!("{name}/PKGBUILD", name = aur_package.name))
            .ok_or_else(|| anyhow!("PKGBUILD for {} not found", aur_package.name))?;

        let pkgbuild = Pkgbuild::read(elevate, pkgbuild.contents())?;

        for makedepend in &pkgbuild.makedepends {
            // handled via rustup
            if makedepend == "cargo" {
                continue;
            }

            plan.add(
                Owned(format!(
                    "install makedepend {} for {}",
                    makedepend, aur_package.name
                )),
                elevate,
                Box::new(Packages {
                    install: Owned(vec![Owned(makedepend.clone())]),
                    reason: InstallReason::Explicit,
                }),
            )?;

            plan.register_packages(&Owned(vec![Owned(makedepend.to_owned())]))?;
        }

        for checkdepend in &pkgbuild.checkdepends {
            plan.add(
                Owned(format!(
                    "install checkdepend {} for {}",
                    checkdepend, aur_package.name
                )),
                elevate,
                Box::new(Packages {
                    install: Owned(vec![Owned(checkdepend.clone())]),
                    reason: InstallReason::Explicit,
                }),
            )?;

            plan.register_packages(&Owned(vec![Owned(checkdepend.to_owned())]))?;
        }

        let build_directory = makepkg_build_directory.join(aur_package.name);

        plan.add(
            Owned(format!("create build directory for {}", aur_package.name)),
            elevate,
            Box::new(Directory {
                path: Owned(build_directory.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: makepkg_user.clone(),
                    group: makepkg_group.clone(),
                    mode: FileMode(0o700),
                },
            }),
        )?;

        let gnupg_directory = build_directory.join("gnupg");

        plan.add(
            Owned(format!("create gpg directory for {}", aur_package.name)),
            elevate,
            Box::new(Directory {
                path: Owned(gnupg_directory.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: makepkg_user.clone(),
                    group: makepkg_group.clone(),
                    mode: FileMode(0o700),
                },
            }),
        )?;

        plan.add(
            Owned(format!("create env file for {}", aur_package.name)),
            elevate,
            Box::new(File {
                path: Owned(build_directory.join("env")),
                permissions: FilePermissions {
                    owner: makepkg_user.clone(),
                    group: makepkg_group.clone(),
                    mode: FileMode(0o600),
                },
                on_directory: DirectoryHandling::Bail,
                on_symlink: SymlinkHandling::Proceed,
                binary: true,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Owned(
                    textwrap::dedent(&format!(
                        r#"
                        export GNUPGHOME="{gnupg_directory}"
                        "#,
                    ))
                    .into_bytes(),
                )),
            }),
        )?;

        if let Some(ref preexec) = aur_package.preexec {
            plan.add(
                Owned(format!("create preexec script for {}", aur_package.name)),
                elevate,
                Box::new(File {
                    path: Owned(build_directory.join("preexec")),
                    permissions: FilePermissions {
                        owner: makepkg_user.clone(),
                        group: makepkg_group.clone(),
                        mode: FileMode(0o700),
                    },
                    on_directory: DirectoryHandling::Bail,
                    on_symlink: SymlinkHandling::Proceed,
                    binary: true,
                    ignore_checksum_changes: false,
                    content: FileContent::Bytes(Owned(preexec.clone().into_bytes())),
                }),
            )?;
        }

        plan.add(
            Owned(format!("create build script for {}", aur_package.name)),
            elevate,
            Box::new(File {
                path: Owned(build_directory.join("build.sh")),
                permissions: FilePermissions {
                    owner: makepkg_user.clone(),
                    group: makepkg_group.clone(),
                    mode: FileMode(0o700),
                },
                on_directory: DirectoryHandling::Bail,
                on_symlink: SymlinkHandling::Proceed,
                binary: true,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Owned(
                    textwrap::dedent(&format!(
                        r#"
                        #!/usr/bin/env bash

                        set -x

                        cd /var/lib/makepkg/build/{name}/

                        if [[ -e ./preexec ]] ; then
                            sh ./preexec || exit 1
                        fi

                        source /var/lib/makepkg/build/{name}/env

                        export PKGEXT='.pkg.tar'
                        export BUILDDIR=/var/lib/makepkg/build/{name}/build/
                        export SRCDEST=/var/lib/makepkg/build/{name}/src/
                        export PKGDEST=/var/lib/makepkg/build/{name}/

                        cd /var/lib/makepkg/pkgbuilds/{name}/

                        source ./PKGBUILD

                        for a in "${{arch[@]}}" ; do
                          if [[ "${{a}}" == "any" ]] ; then
                            arch="any"
                            break
                          fi
                          if [[ "${{a}}" == "x86_64" ]] ; then
                            arch="x86_64"
                            break
                          fi
                        done

                        if [[ ! "${{arch}}" ]] ; then
                          printf 'unsupported arch\n' >&2
                          exit 1
                        fi

                        if [[ "${{epoch}}" ]] ; then
                          version="${{epoch}}:${{pkgver}}-${{pkgrel}}"
                        else
                          version="${{pkgver}}-${{pkgrel}}"
                        fi

                        filename="${{PKGDEST%/}}/${{pkgname}}-${{version}}-${{arch}}${{PKGEXT}}"

                        if [[ ! -e "${{filename}}" ]] ; then
                          makepkg \
                            --clean \
                            --nosign || exit 1
                        fi

                        printf '%s' "${{filename}}" > /var/lib/makepkg/build/{name}/pkgname

                        rm -rf "${{BUILDDIR}}"
                        rm -rf "${{SRCDEST}}"
                        "#,
                        name = aur_package.name,
                    ))
                    .trim()
                    .to_owned()
                    .into_bytes(),
                )),
            }),
        )?;

        let install_script = build_directory.join("install.sh");

        plan.add(
            Owned(format!("create install script for {}", aur_package.name)),
            elevate,
            Box::new(File {
                path: Owned(install_script.clone()),
                permissions: FilePermissions {
                    owner: makepkg_user.clone(),
                    group: makepkg_group.clone(),
                    mode: FileMode(0o700),
                },
                on_directory: DirectoryHandling::Bail,
                on_symlink: SymlinkHandling::Proceed,
                binary: true,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Owned(
                    textwrap::dedent(&format!(
                        r#"
                        #!/usr/bin/env bash

                        sudo -u makepkg -g makepkg /var/lib/makepkg/build/{name}/build.sh || exit 1

                        filename="$(</var/lib/makepkg/build/{name}/pkgname)"

                        pacman --upgrade --needed --noconfirm "$filename" || exit 1
                        "#,
                        name = aur_package.name,
                    ))
                    .trim()
                    .to_owned()
                    .into_bytes(),
                )),
            }),
        );

        let version = get_package_version(aur_package.name, elevate)?;

        if !version.is_some_and(|version| {
            version
                == match pkgbuild.epoch {
                    None => format!(
                        "{pkgver}-{pkgrel}",
                        pkgver = pkgbuild.pkgver,
                        pkgrel = pkgbuild.pkgrel
                    ),
                    Some(epoch) => format!(
                        "{epoch}:{pkgver}-{pkgrel}",
                        pkgver = pkgbuild.pkgver,
                        pkgrel = pkgbuild.pkgrel
                    ),
                }
        }) {
            plan.add(
                Owned(format!("build aur package {}", aur_package.name)),
                elevate,
                Box::new(Command {
                    cmd: Owned(install_script.into_string()),
                    args: &[],
                    check_command: None,
                }),
            )?;
        }

        plan.register_packages(&Owned(vec![Owned(aur_package.name.to_owned())]));
    }

    Ok(())
}

fn boot_target<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
    config: &Config,
) -> Result<()> {
    plan.add(
        Ref("setting boot target"),
        elevate,
        Box::new(BootTarget {
            target: config.machine.boot_target,
        }),
    )?;

    Ok(())
}

fn sudo<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
) -> Result<()> {
    plan.add(
        Ref("add sudonopw"),
        elevate,
        Box::new(Group {
            name: "sudonopw",
            system: true,
        }),
    )?;

    plan.add(
        Ref("configure passwordless sudo"),
        elevate,
        Box::new(File {
            path: Ref(Path::new("/etc/sudoers.d/sudonopw")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o600),
            },
            on_directory: DirectoryHandling::Bail,
            on_symlink: SymlinkHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: Lines(Owned(vec![Ref("%sudonopw ALL=(ALL) NOPASSWD: ALL")])),
        }),
    )?;

    plan.add(
        Ref("configure passwordless doas"),
        elevate,
        Box::new(File {
            path: Ref(Path::new("/etc/doas.conf")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o400),
            },
            on_directory: DirectoryHandling::Bail,
            on_symlink: SymlinkHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: Lines(Owned(vec![Ref("permit nopass nolog setenv {PATH=/usr/local/bin:/usr/local/sbin:/usr/bin:/usr/sbin} :sudonopw")])),
        }),
    )?;

    Ok(())
}

fn timesyncd<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
) -> Result<()> {
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
        Box::new(File {
            path: Ref(Path::new("/etc/systemd/timesyncd.conf")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o644),
            },
            on_directory: DirectoryHandling::Bail,
            on_symlink: SymlinkHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: Lines(Owned(vec![
                Ref("[Time]"),
                Owned(format!("NTP={}", arch_ntp_servers.join(" "))),
                Owned(format!("FallbackNTP={}", fallback_ntp_servers.join(" "))),
            ])),
        }),
    )?;

    Ok(())
}

fn reflector<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
) -> Result<()> {
    plan.add(
        "configure reflector",
        elevate,
        Box::new(File {
            path: Ref(Path::new("/etc/xdg/reflector/reflector.conf")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o644),
            },
            on_directory: DirectoryHandling::Bail,
            on_symlink: SymlinkHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: Lines(Ref(&[
                Ref("--save /etc/pacman.d/mirrorlist"),
                Ref("--protocol https"),
                Ref("--country Germany"),
                Ref("--latest 5"),
                Ref("--sort age"),
            ])),
        }),
    )?
    .on_change(Box::new(|| {
        Ok(Resource::new(
            Ref("restart reflector"),
            elevate,
            Box::new(RestartService {
                name: "reflector.service",
            }),
        ))
    }))?;

    plan.add(
        "enable reflector timer",
        elevate,
        Box::new(Service {
            name: "reflector.timer",
            state: ServiceState {
                runtime: ServiceStateRuntime::Active,
                persistent: ServiceStatePersistent::Enabled,
            },
        }),
    )?;
    Ok(())
}

fn pacman<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
    config: &Config,
) -> Result<()> {
    plan.add(
        "configure pacman",
        elevate,
        Box::new(File {
            path: Ref(Path::new("/etc/pacman.conf")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o644),
            },
            on_symlink: SymlinkHandling::Proceed,
            on_directory: DirectoryHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: FileContent::Bytes(Owned(
                textwrap::dedent(
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
                    "##,
                )
                .into_bytes(),
            )),
        }),
    )?
    .on_change(Box::new(|| {
        Ok(Resource::new(
            Ref("refresh package database"),
            elevate,
            Box::new(PackageDatabaseRefresh {}),
        ))
    }))?;

    Ok(())
}

fn pacman_cache_cleanup<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
) -> Result<()> {
    let pacman_contrib = &[Ref("pacman-contrib")];
    plan.add(
        "install pacman-contrib",
        elevate,
        Box::new(Packages {
            install: Ref(pacman_contrib),
            reason: InstallReason::Explicit,
        }),
    )?;

    plan.register_packages(&Ref(pacman_contrib));

    plan.add(
        "pacman cache cleanup service",
        elevate,
        Box::new(File {
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
            ignore_checksum_changes: false,
            content: Lines(Ref(&[
                Ref("[Service]"),
                Ref("Type=oneshot"),
                Ref("ExecStart=/bin/sh -c '/usr/bin/paccache -rk1 && /usr/bin/paccache -ruk0'"),
                Ref("RemainAfterExit=true"),
            ])),
        }),
    )?;

    plan.add(
        "pacman cache cleanup timer",
        elevate,
        Box::new(File {
            path: Ref(Path::new("/etc/systemd/system/pacman-cache-cleanup.timer")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o644),
            },
            on_directory: DirectoryHandling::Bail,
            on_symlink: SymlinkHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: Lines(Ref(&[
                Ref("[Timer]"),
                Ref("OnCalendar=daily"),
                Ref(""),
                Ref("[Install]"),
                Ref("WantedBy=multi-user.target"),
            ])),
        }),
    )?;

    plan.add(
        "enable pacman cache cleanup timer",
        elevate,
        Box::new(Service {
            name: "pacman-cache-cleanup.timer",
            state: ServiceState {
                runtime: ServiceStateRuntime::Active,
                persistent: ServiceStatePersistent::Enabled,
            },
        }),
    )?;
    Ok(())
}

fn install_packages<'elevate: 'action, 'action, 'pkgs>(
    plan: &mut Plan<'_, 'elevate, 'pkgs, 'action>,
    elevate: &'elevate Elevate,
    packages: &'pkgs PackageConfig,
) -> Result<()> {
    let mut changes = false;

    for &(section, packages) in packages.0 {
        let packages = packages
            .iter()
            .map(|package| Ref(*package))
            .collect::<Vec<_>>();

        let action = plan.add(
            Owned(format!("install packages for {section}")),
            elevate,
            Box::new(Packages {
                install: Owned(packages.clone()),
                reason: InstallReason::Explicit,
            }),
        )?;

        plan.register_packages(&Owned(packages));
    }

    Ok(())
}

fn hardware<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
    config: &Config,
) -> Result<()> {
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
                Box::new(Packages {
                    install: Ref(amdgpu_packages),
                    reason: InstallReason::Explicit,
                }),
            )?;

            plan.register_packages(&Ref(amdgpu_packages));

            plan.add(
                "set AMDGPU options",
                elevate,
                Box::new(File {
                    path: Ref(Path::new("/etc/X11/xorg.conf.d/20-amdgpu.conf")),
                    permissions: FilePermissions {
                        owner: Uid::root(),
                        group: Gid::root(),
                        mode: FileMode(0o600),
                    },
                    on_directory: DirectoryHandling::Bail,
                    on_symlink: SymlinkHandling::Proceed,
                    binary: false,
                    ignore_checksum_changes: false,
                    content: Lines(Ref(&[
                        Ref(r#"Section "Device""#),
                        Ref(r#"    Identifier "AMD""#),
                        Ref(r#"    Driver "amdgpu""#),
                        Ref(r#"    Option "VariableRefresh" "true""#),
                        Ref(r#"    Option "TearFree" "true""#),
                        Ref(r#"EndSection"#),
                    ])),
                }),
            )?;
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
                Box::new(Packages {
                    install: Ref(nouveau_packages),
                    reason: InstallReason::Explicit,
                }),
            )?;

            plan.register_packages(&Ref(nouveau_packages));
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
                Box::new(Packages {
                    install: Ref(intel_packages),
                    reason: InstallReason::Explicit,
                }),
            )?;

            plan.register_packages(&Ref(intel_packages));
        }
    }

    match config.machine.cpu {
        Cpu::Amd => {
            let amd_cpu_packages = &[Ref("amd-ucode")];

            plan.add(
                "install AMD cpu packages",
                elevate,
                Box::new(Packages {
                    install: Ref(amd_cpu_packages),
                    reason: InstallReason::Explicit,
                }),
            )?;

            plan.register_packages(&Ref(amd_cpu_packages));
        }
        Cpu::Intel => {
            let intel_cpu_packages = &[Ref("intel-ucode")];

            plan.add(
                "install intel cpu packages",
                elevate,
                Box::new(Packages {
                    install: Ref(intel_cpu_packages),
                    reason: InstallReason::Explicit,
                }),
            )?;

            plan.register_packages(&Ref(intel_cpu_packages));
        }
    }

    Ok(())
}

fn backlight<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
) -> Result<()> {
    plan.add(
        // See https://wiki.archlinux.org/title/backlight#ACPI
        Ref("create udev rule to allow video group backlight access"),
        elevate,
        Box::new(File {
            path: Ref(Path::new("/etc/udev/rules.d/backlight.rules")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o644),
            },
            on_directory: DirectoryHandling::Bail,
            on_symlink: SymlinkHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: Lines(Ref(&[Ref(r#"ACTION=="add", SUBSYSTEM=="backlight", RUN+="/bin/chgrp video $sys$devpath/brightness", RUN+="/bin/chmod g+w $sys$devpath/brightness""#)])),
        }),
    )?;

    Ok(())
}

fn kernel_modules<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
) -> Result<()> {
    plan.add(
        "disable kernel modules",
        elevate,
        Box::new(File {
            path: Ref(Path::new("/etc/modprobe.d/disable.conf")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o600),
            },
            on_directory: DirectoryHandling::Bail,
            on_symlink: SymlinkHandling::Proceed,
            binary: false,
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
        }),
    )?;

    Ok(())
}

fn journald<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
) -> Result<()> {
    plan.add(
        "limit journald size",
        elevate,
        Box::new(LineInFile {
            path: Path::new("/etc/systemd/journald.conf"),
            line: Ref("SystemMaxUse=50M"),
            regex: Regex::new(r#"^#?SystemMaxUse=.*$"#).expect("regex to be valid"),
        }),
    )?
    .on_change(Box::new(|| {
        Ok(Resource::new(
            Ref("restart journald"),
            elevate,
            Box::new(RestartService {
                name: "systemd-journald.service",
            }),
        ))
    }))?;
    Ok(())
}

fn hardware_switches<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
) -> Result<()> {
    plan.add(
        "handle lid switch",
        elevate,
        Box::new(LineInFile {
            path: Path::new("/etc/systemd/logind.conf"),
            line: Ref("HandleLidSwitch=ignore"),
            regex: Regex::new(r#"^#?HandleLidSwitch=.*$"#).expect("regex to be valid"),
        }),
    )?;
    plan.add(
        "handle power key",
        elevate,
        Box::new(LineInFile {
            path: Path::new("/etc/systemd/logind.conf"),
            line: Ref("HandlePowerKey=suspend"),
            regex: Regex::new(r#"^#?HandlePowerKey=.*$"#).expect("regex to be valid"),
        }),
    )?;
    Ok(())
}

fn firefox<'elevate: 'action, 'action>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    elevate: &'elevate Elevate,
) -> Result<()> {
    for directory in [
        Path::new("/etc/firefox"),
        Path::new("/etc/firefox/policies"),
    ] {
        plan.add(
            Owned(format!("create firefox directories {directory}")),
            elevate,
            Box::new(Directory {
                path: Ref(directory),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::root(),
                    group: Gid::root(),
                    mode: FileMode(0o775),
                },
            }),
        )?;
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
        Box::new(File {
            path: Ref(Path::new("/etc/firefox/policies/policies.json")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o644),
            },
            on_directory: DirectoryHandling::Bail,
            on_symlink: SymlinkHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: FileContent::Bytes(Owned(serde_json::to_vec_pretty(&policies)?)),
        }),
    )?;

    Ok(())
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
    plan: &mut Plan<'_, 'elevate, 'pkgs, 'action>,
    elevate: &'elevate Elevate,
    config: &Config,
    packages: &'pkgs PackageConfig,
) -> Result<()> {
    pacman(plan, elevate, config)?;
    pacman_cache_cleanup(plan, elevate)?;
    install_packages(plan, elevate, packages)?;
    reflector(plan, elevate)?;
    initrd(plan, elevate, config)?;
    libvirt(plan, elevate)?;
    services(plan, elevate)?;
    boot_target(plan, elevate, config)?;
    aur(plan, elevate)?;
    timesyncd(plan, elevate)?;
    sudo(plan, elevate)?;
    hardware(plan, elevate, config)?;
    hardware_switches(plan, elevate)?;
    journald(plan, elevate)?;
    kernel_modules(plan, elevate)?;
    backlight(plan, elevate)?;
    firefox(plan, elevate)?;
    Ok(())
}

fn configure_user<'elevate: 'action, 'action, 'user: 'action, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    root: &'elevate Elevate,
    user_elevate: &'elevate Elevate,
    user: &'user UserConfig,
    machine: &'machine MachineConfig,
) -> Result<()> {
    let home = PathBuf::from("/home/").join(&user.name);

    plan.add(
        "create user",
        root,
        Box::new(User {
            name: &user.name,
            home: Owned(home.clone()),
            primary_group: PrimaryGroupConfig::Automatic,
            additional_groups: &[
                "libvirt",
                "wheel",
                "wireshark",
                "docker",
                "sudonopw",
                "games",
                "kvm",
                "video",
            ],
            shell: "/usr/bin/zsh",
            system: false,
        }),
    )?;

    if user.autologin {
        user_autologin(plan, root, user)?;
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

    user_create_directories(plan, user_elevate, user, &user_directories)?;
    user_executables(plan, user_elevate, user, &user_directories)?;
    user_dotfiles(plan, user_elevate, user, machine, &home)?;
    user_wallpaper(plan, user_elevate, user, &user_directories)?;
    user_services(plan, user_elevate, user, &user_directories, machine)?;
    user_spotify(plan, user_elevate, user, &user_directories)?;
    user_firefox(plan, user_elevate, user, &home)?;

    plan.add(
        "import my gpg key",
        user_elevate,
        Box::new(GpgKey {
            content: include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/gpgkeys/hannes@hkoerber.de.gpg.asc"
            )),
            gnupg_directory: Owned(home.join(".local/state/gnupg/")),
            fingerprint: GpgFingerprint("973AE48D71B76735C4712B5BB5C002530C6A2053".to_owned()),
            trust: GpgTrust::Ultimately,
        }),
    )?;

    Ok(())
}

fn user_firefox<'elevate: 'action, 'action, 'user, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    user_elevate: &'elevate Elevate,
    user: &'user UserConfig,
    home: &Path,
) -> Result<()> {
    let mozilla_dir = home.join(".mozilla");
    let firefox_dir = mozilla_dir.join("firefox");

    plan.add(
        Ref("create mozilla base directory"),
        user_elevate,
        Box::new(Directory {
            path: Owned(mozilla_dir),
            on_symlink: SymlinkHandling::Proceed,
            on_file: FileHandling::Proceed,
            permissions: FilePermissions {
                owner: Uid::from(user.name.clone()),
                group: Gid::from(user.name.clone()),
                mode: FileMode(0o755),
            },
        }),
    )?;

    plan.add(
        Ref("create firefox base directory"),
        user_elevate,
        Box::new(Directory {
            path: Owned(firefox_dir.clone()),
            on_symlink: SymlinkHandling::Proceed,
            on_file: FileHandling::Proceed,
            permissions: FilePermissions {
                owner: Uid::from(user.name.clone()),
                group: Gid::from(user.name.clone()),
                mode: FileMode(0o755),
            },
        }),
    )?;

    for profile in &user.firefox.profiles {
        let profile_dir = firefox_dir.join(format!("profile-{}", profile.name));
        let chrome_dir = profile_dir.join("chrome");

        plan.add(
            Owned(format!(
                "create firefox profile directory for {}",
                profile.name
            )),
            user_elevate,
            Box::new(Directory {
                path: Owned(profile_dir),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            }),
        )?;

        plan.add(
            Owned(format!(
                "create firefox chrome directory for {}",
                profile.name
            )),
            user_elevate,
            Box::new(Directory {
                path: Owned(chrome_dir.clone()),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
            }),
        )?;

        if profile.manage_css {
            plan.add(
            Owned(format!(
                "create firefox userChrome.css for {}",
                profile.name
            )),
            user_elevate,
            Box::new(File {
                path: Owned(chrome_dir.join("userChrome.css")),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
            binary: false,
            ignore_checksum_changes: false,
                content: FileContent::Bytes(Owned(
                    textwrap::dedent(
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
                        "##,
                    )
                    .into_bytes(),
                )),
            }),
        )?;
        }
    }

    Ok(())
}

fn user_create_directories<'elevate: 'action, 'action, 'user, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    user_elevate: &'elevate Elevate,
    user: &'user UserConfig,
    user_directories: &UserDirectories,
) -> Result<()> {
    plan.add(
        "create user config directory",
        user_elevate,
        Box::new(Directory {
            path: Owned(user_directories.config.clone()),
            on_symlink: SymlinkHandling::Proceed,
            on_file: FileHandling::Proceed,
            permissions: FilePermissions {
                owner: Uid::from(user.name.clone()),
                group: Gid::from(user.name.clone()),
                mode: FileMode(0o755),
            },
        }),
    )?;

    plan.add(
        Ref("create download directory"),
        user_elevate,
        Box::new(Directory {
            path: Owned(user_directories.download.clone()),
            on_symlink: SymlinkHandling::Proceed,
            on_file: FileHandling::Proceed,
            permissions: FilePermissions {
                owner: Uid::from(user.name.clone()),
                group: Gid::from(user.name.clone()),
                mode: FileMode(0o755),
            },
        }),
    )?;

    plan.add(
        Ref("create tmp directory"),
        user_elevate,
        Box::new(Directory {
            path: Owned(user_directories.tmp.clone()),
            on_symlink: SymlinkHandling::Proceed,
            on_file: FileHandling::Proceed,
            permissions: FilePermissions {
                owner: Uid::from(user.name.clone()),
                group: Gid::from(user.name.clone()),
                mode: FileMode(0o755),
            },
        }),
    )?;

    plan.add(
        Ref("create scripts directory"),
        user_elevate,
        Box::new(Directory {
            path: Owned(user_directories.scripts.clone()),
            on_symlink: SymlinkHandling::Proceed,
            on_file: FileHandling::Proceed,
            permissions: FilePermissions {
                owner: Uid::from(user.name.clone()),
                group: Gid::from(user.name.clone()),
                mode: FileMode(0o755),
            },
        }),
    )?;

    plan.add(
        Ref("create bin directory"),
        user_elevate,
        Box::new(Directory {
            path: Owned(user_directories.bin.clone()),
            on_symlink: SymlinkHandling::Proceed,
            on_file: FileHandling::Proceed,
            permissions: FilePermissions {
                owner: Uid::from(user.name.clone()),
                group: Gid::from(user.name.clone()),
                mode: FileMode(0o755),
            },
        }),
    )?;

    plan.add(
        Ref("create systemd user directory"),
        user_elevate,
        Box::new(Directory {
            path: Owned(user_directories.systemd.clone()),
            on_symlink: SymlinkHandling::Proceed,
            on_file: FileHandling::Proceed,
            permissions: FilePermissions {
                owner: Uid::from(user.name.clone()),
                group: Gid::from(user.name.clone()),
                mode: FileMode(0o755),
            },
        }),
    )?;

    plan.add(
        Ref("create systemd user unit directory"),
        user_elevate,
        Box::new(Directory {
            path: Owned(user_directories.systemd_user_units.clone()),
            on_symlink: SymlinkHandling::Proceed,
            on_file: FileHandling::Proceed,
            permissions: FilePermissions {
                owner: Uid::from(user.name.clone()),
                group: Gid::from(user.name.clone()),
                mode: FileMode(0o755),
            },
        }),
    )?;

    plan.add(
        Ref("create local directory"),
        user_elevate,
        Box::new(Directory {
            path: Owned(user_directories.local.clone()),
            on_symlink: SymlinkHandling::Proceed,
            on_file: FileHandling::Proceed,
            permissions: FilePermissions {
                owner: Uid::from(user.name.clone()),
                group: Gid::from(user.name.clone()),
                mode: FileMode(0o755),
            },
        }),
    )?;

    plan.add(
        Ref("create data directory"),
        user_elevate,
        Box::new(Directory {
            path: Owned(user_directories.data.clone()),
            on_symlink: SymlinkHandling::Proceed,
            on_file: FileHandling::Proceed,
            permissions: FilePermissions {
                owner: Uid::from(user.name.clone()),
                group: Gid::from(user.name.clone()),
                mode: FileMode(0o755),
            },
        }),
    )?;

    Ok(())
}

fn user_spotify<'elevate: 'action, 'action, 'user, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    user_elevate: &'elevate Elevate,
    user: &'user UserConfig,
    user_directories: &UserDirectories,
) -> Result<()> {
    plan.add(
        Owned("fix spotify audio api".to_owned()),
        user_elevate,
        Box::new(File {
            path: Owned(user_directories.config.join("spotify-flags.conf")),
            on_symlink: SymlinkHandling::Proceed,
            on_directory: DirectoryHandling::Bail,
            permissions: FilePermissions {
                owner: Uid::from(user.name.clone()),
                group: Gid::from(user.name.clone()),
                mode: FileMode(0o644),
            },
            binary: false,
            ignore_checksum_changes: false,
            content: FileContent::Lines(Ref(&[Ref("--audio-api=pulseaudio")])),
        }),
    )?;
    Ok(())
}

fn user_autologin<'elevate: 'action, 'action, 'user, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    root: &'elevate Elevate,
    user: &'user UserConfig,
) -> Result<()> {
    let getty_base = PathBuf::from(format!(
        "/etc/systemd/system/getty@tty{vt}.service.d/",
        vt = user.vt
    ));

    plan.add(
        "create directory for getty autologin",
        root,
        Box::new(Directory {
            path: Owned(getty_base.clone()),
            on_symlink: SymlinkHandling::Proceed,
            on_file: FileHandling::Proceed,
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o755),
            },
        }),
    )?;

    plan.add(
        "enable getty autologin",
        root,
        Box::new(File {
            path: Owned(getty_base.join("override.conf")),
            permissions: FilePermissions {
                owner: Uid::root(),
                group: Gid::root(),
                mode: FileMode(0o644),
            },
            on_directory: DirectoryHandling::Bail,
            on_symlink: SymlinkHandling::Proceed,
            binary: false,
            ignore_checksum_changes: false,
            content: Lines(Owned(vec![
                Ref("[Service]"),
                Ref("ExecStart="),
                Owned(format!(
                    "ExecStart=-/sbin/agetty --autologin {name} --noclear %I $TERM",
                    name = user.name
                )),
            ])),
        }),
    )?;

    Ok(())
}

fn user_dotfiles<'elevate: 'action, 'action, 'user, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
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
        (".local/share/", None),
        (".local/share/applications", None),
        (".config/VSCodium/", None),
        (".config/VSCodium/User", None),
        (".config/alacritty/themes/", None),
    ] {
        plan.add(
            Owned(format!("create empty directory {directory}")),
            user_elevate,
            Box::new(Directory {
                path: Owned(home.join(directory)),
                on_symlink: SymlinkHandling::Proceed,
                on_file: FileHandling::Proceed,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: mode.unwrap_or(FileMode(0o755)),
                },
            }),
        )?;
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
            Box::new(Remove {
                path: Owned(home.join(to_remove)),
                on_directory: DirectoryHandling::Proceed,
                on_file: FileHandling::Proceed,
                on_symlink: SymlinkHandling::Proceed,
            }),
        )?;
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
            Box::new(File {
                path: Owned(home.join(path)),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(content)),
            }),
        )?;
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
            Box::new(File {
                path: Owned(home.join(path)),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Owned(template.into_bytes())),
            }),
        )?;
    }

    Ok(())
}

fn user_executables<'elevate: 'action, 'action, 'user, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    user_elevate: &'elevate Elevate,
    user: &'user UserConfig,
    user_directories: &UserDirectories,
) -> Result<()> {
    for (script, content) in [
        (
            "fix-aoe2.sh",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/scripts/fix-aoe2.sh"
            ))
            .as_slice(),
        ),
        (
            "garmin.sh",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/scripts/garmin.sh"
            ))
            .as_slice(),
        ),
        (
            "install-gothic.sh",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/scripts/install-gothic.sh"
            ))
            .as_slice(),
        ),
        (
            "install-gothic2.sh",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/scripts/install-gothic2.sh"
            ))
            .as_slice(),
        ),
    ] {
        plan.add(
            Owned(format!("add script {script}")),
            user_elevate,
            Box::new(File {
                path: Owned(user_directories.scripts.join(script)),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
                binary: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(content)),
            }),
        )?;
    }

    for (bin, content) in [
        (
            "ascii",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../files/bin/ascii")).as_slice(),
        ),
        (
            "asciihex",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/bin/asciihex"
            ))
            .as_slice(),
        ),
        (
            "days-until",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/bin/days-until"
            ))
            .as_slice(),
        ),
        (
            "firefox-default",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/bin/firefox-default"
            ))
            .as_slice(),
        ),
        (
            "git-rewrite-author",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/bin/git-rewrite-author"
            ))
            .as_slice(),
        ),
        (
            "gpg-yubikey-rescan",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/bin/gpg-yubikey-rescan"
            ))
            .as_slice(),
        ),
        (
            "kw",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../files/bin/kw")).as_slice(),
        ),
        (
            "markdown-to-pdf",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/bin/markdown-to-pdf"
            ))
            .as_slice(),
        ),
        (
            "mousespam",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/bin/mousespam"
            ))
            .as_slice(),
        ),
        (
            "mousespam-toggle",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/bin/mousespam-toggle"
            ))
            .as_slice(),
        ),
        (
            "nato",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../files/bin/nato")).as_slice(),
        ),
        (
            "natode",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../files/bin/natode")).as_slice(),
        ),
        (
            "notify",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../files/bin/notify")).as_slice(),
        ),
        (
            "pacman-remove-orphans",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/bin/pacman-remove-orphans"
            ))
            .as_slice(),
        ),
        (
            "paselect",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/bin/paselect"
            ))
            .as_slice(),
        ),
        (
            "pass",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../files/bin/pass")).as_slice(),
        ),
        (
            "pf",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../files/bin/pf")).as_slice(),
        ),
        (
            "pprint",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../files/bin/pprint")).as_slice(),
        ),
        (
            "pw",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../files/bin/pw")).as_slice(),
        ),
        (
            "qr",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../files/bin/qr")).as_slice(),
        ),
        (
            "randmac",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../files/bin/randmac")).as_slice(),
        ),
        (
            "split-multiyaml-to-json",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/bin/split-multiyaml-to-json"
            ))
            .as_slice(),
        ),
        (
            "switch-color-mode",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/bin/switch-color-mode"
            ))
            .as_slice(),
        ),
        (
            "terraform-get-targets",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/bin/terraform-get-targets"
            ))
            .as_slice(),
        ),
        (
            "unascii",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../files/bin/unascii")).as_slice(),
        ),
        (
            "yaml2json",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../files/bin/yaml2json"
            ))
            .as_slice(),
        ),
    ] {
        plan.add(
            Owned(format!("add executable {bin}")),
            user_elevate,
            Box::new(File {
                path: Owned(user_directories.bin.join(bin)),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o755),
                },
                binary: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(content)),
            }),
        )?;
    }

    Ok(())
}

fn user_wallpaper<'elevate: 'action, 'action, 'user, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    user_elevate: &'elevate Elevate,
    user: &'user UserConfig,
    user_directories: &UserDirectories,
) -> Result<()> {
    plan.add(
        Ref("install wallpaper"),
        user_elevate,
        Box::new(File {
            path: Owned(user_directories.data.join("wallpaper.jpg")),
            on_symlink: SymlinkHandling::Proceed,
            on_directory: DirectoryHandling::Bail,
            permissions: FilePermissions {
                owner: Uid::from(user.name.clone()),
                group: Gid::from(user.name.clone()),
                mode: FileMode(0o644),
            },
            binary: true,
            ignore_checksum_changes: false,
            content: FileContent::Bytes(Ref(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../assets/wallpaper.jpg"
            )))),
        }),
    )?;

    Ok(())
}
fn user_services<'elevate: 'action, 'action, 'user, 'machine>(
    plan: &mut Plan<'_, 'elevate, '_, 'action>,
    user_elevate: &'elevate Elevate,
    user: &'user UserConfig,
    user_directories: &UserDirectories,
    machine: &MachineConfig,
) -> Result<()> {
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
                Box::new(File {
                    path,
                    on_symlink: SymlinkHandling::Proceed,
                    on_directory: DirectoryHandling::Bail,
                    permissions: FilePermissions {
                        owner: Uid::from(user.name.clone()),
                        group: Gid::from(user.name.clone()),
                        mode: FileMode(0o644),
                    },
                    binary: false,
                    ignore_checksum_changes: false,
                    content: FileContent::Bytes(Ref(content)),
                }),
            )?;
        } else {
            plan.add(
                Owned(format!("user service {unit}")),
                user_elevate,
                Box::new(Remove {
                    path,
                    on_directory: DirectoryHandling::Bail,
                    on_file: FileHandling::Proceed,
                    on_symlink: SymlinkHandling::Proceed,
                }),
            )?;
        }
    }

    plan.add(
        "autostart target",
        user_elevate,
        Box::new(File {
            path: Owned(user_directories.systemd_user_units.join("autostart.target")),
            on_symlink: SymlinkHandling::Proceed,
            on_directory: DirectoryHandling::Bail,
            permissions: FilePermissions {
                owner: Uid::from(user.name.clone()),
                group: Gid::from(user.name.clone()),
                mode: FileMode(0o644),
            },
            binary: false,
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
        }),
    )?;

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
            Box::new(File {
                path: Owned(user_directories.systemd_user_units.join(unit)),
                on_symlink: SymlinkHandling::Proceed,
                on_directory: DirectoryHandling::Bail,
                permissions: FilePermissions {
                    owner: Uid::from(user.name.clone()),
                    group: Gid::from(user.name.clone()),
                    mode: FileMode(0o644),
                },
                binary: false,
                ignore_checksum_changes: false,
                content: FileContent::Bytes(Ref(content)),
            }),
        )?;
    }

    Ok(())
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

impl UserConfig {
    fn get_elevate(&self) -> Elevate {
        Elevate::Sudo(ElevateTo::User {
            user: self.name.clone(),
        })
    }
}

struct Config {
    machine: MachineConfig,
    users: Vec<(Elevate, UserConfig)>,
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

fn main() -> Result<()> {
    let args = Cli::try_parse()?;

    let subscriber = configure_tracing(if args.debug {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    });

    tracing::subscriber::set_global_default(subscriber)?;

    let running_user = Process::run("id", &["--user", "--name"], &Elevate::No, None)?
        .check()?
        .stdout
        .trim()
        .to_owned();

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
            machine_type: Laptop,
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
    .map(|user| {
        (
            match running_user.as_str() {
                "root" => Elevate::No,
                name if name == user.name => Elevate::No,
                other => Elevate::Sudo(ElevateTo::User {
                    user: user.name.clone(),
                }),
            },
            user,
        )
    })
    .collect();

    let config = Config { machine, users };

    let mut plan = Plan::new();

    root_setup(&mut plan, &root, &config, &PACKAGES)?;

    for user in &config.users {
        let (ref user_elevate, ref user) = *user;

        configure_user(&mut plan, &root, user_elevate, user, &config.machine)?;
    }

    match args.hostname {
        Hera => {
            hera(&mut plan, &root, &config)?;
            autoupdate(&mut plan, &root, &config)?;
            restic_backup(&mut plan, &root, &config)?;
        }
        Dionysus => {
            autoupdate(&mut plan, &root, &config)?;
        }
        Ares | Neptune => (),
    }

    unmanaged_packages(&mut plan, &root, &config)?;
    orphans(&mut plan, &root, &config)?;

    let plan_finished = plan.finish()?;

    let report = plan_finished.report()?;
    report.print(&if args.only_changed {
        ReportConfig::OnlyChanged
    } else {
        ReportConfig::All
    });

    let has_changes = report.has_changes();

    let plan_approved = plan_finished.approve();

    if !args.dry_run && has_changes {
        plan_approved.apply()?;
    }

    Ok(())
}
