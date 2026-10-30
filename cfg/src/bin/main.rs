#![allow(unused, reason = "dev")]
// required for `serde_json::json!()`
#![recursion_limit = "256"]

use std::{collections::HashMap, env, fs};

use anyhow::{Result, bail};
use clap::{Parser, ValueEnum};
use regex::Regex;
use serde::{Deserialize, Serialize};

use cfg::prelude::*;

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
    ("git", &["git", "tk", "git-delta", "git-filter-repo"]),
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
    ("firefox", &["firefox"]),
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

fn hera<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
    config: &'ctx Config,
) -> Result<()> {
    plan.add(
        "install packages",
        ctx,
        Box::new(Packages {
            install: Ref(&[
                "plasma-desktop",
                "konsole",
                "dolphin",
                "kdeplasma-addons",
                "plasma-nm",
                "plasma-pa",
                "plasma-systemmonitor",
                "sddm",
                "sddm-kcm",
                "thunderbird",
                // kde archive manager
                "ark",
                // kde image viewer
                "gwenview",
                // german language packs
                "hunspell-de",
                "thunderbird-i18n-de",
                "firefox-i18n-de",
            ]),
        }),
    )?;

    plan.add(
        Ref("create user"),
        ctx,
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
        ctx,
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
        ctx,
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
        ctx,
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
        ctx,
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

fn autoupdate<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
    config: &'ctx Config,
) -> Result<()> {
    plan.add(
        "deploy autoupdate script",
        ctx,
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
            content: FileContent::Bytes(Owned(
                textwrap::dedent(
                    r#"
                    #!/usr/bin/env bash

                    set -o errexit
                    set -o nounset
                    set -o pipefail

                    # Prevent failures when not battery present
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

    let serivce_or_timer_changed = if plan
        .add(
            "install pacman autoupdate service",
            ctx,
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
        .changed()
    {
        true
    } else {
        plan.add(
            "install pacman autoupdate timer",
            ctx,
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
        .changed()
    };

    if serivce_or_timer_changed {
        plan.add(
            "restart service manager due to new packages",
            ctx,
            Box::new(DaemonReload {}),
        )?;
    }

    plan.add(
        Ref("enable pacman autoupdate timer"),
        ctx,
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

fn restic_backup<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
    config: &'ctx Config,
) -> Result<()> {
    plan.add(
        Ref("create restic config directory"),
        ctx,
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
        ctx,
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
        ctx,
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
        ctx,
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
        ctx,
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

    let serivce_or_timer_changed = if plan
        .add(
            "install restic backup service",
            ctx,
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
        .changed()
    {
        true
    } else {
        plan.add(
            "install restic backup timer",
            ctx,
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
        .changed()
    };

    if serivce_or_timer_changed {
        plan.add(
            "restart service manager due to new packages",
            ctx,
            Box::new(DaemonReload {}),
        )?;
    }

    plan.add(
        Ref("enable pacman autoupdate timer"),
        ctx,
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

fn initrd<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
    config: &'ctx Config,
) -> Result<()> {
    plan.add(
        "install lz4",
        ctx,
        Box::new(Packages {
            install: Ref(&["lz4"]),
        }),
    )?;

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
        cmd: "mkinitcpio",
        args: &["-P"],
    });

    plan.add(
        "configure mkinitcpio",
        ctx,
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
    .on_change(|plan| {
        plan.add("rebuild initrd", ctx, rebuild_mkinitcpio)?;
        Ok(())
    })?;

    Ok(())
}

fn libvirt<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
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
            ctx,
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
        ctx,
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
            content: Lines(Ref(&[Ref("LIBVIRTD_ARGS=")])),
        }),
    )?
    .on_change(|plan| {
        plan.add(
            "restart libvirt",
            ctx,
            Box::new(RestartService {
                name: "libvirtd.service",
            }),
        )?;
        Ok(())
    })?;

    plan.add(
        Ref("enable libvirt"),
        ctx,
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

fn services<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
) -> Result<()> {
    #[expect(
        clippy::single_element_loop,
        reason = "consistency with enabled services"
    )]
    for service in ["sshd.service"] {
        plan.add(
            Owned(format!("disable service {service}")),
            ctx,
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
            ctx,
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
            ctx,
            Box::new(Service {
                name: socket,
                state: ServiceState {
                    runtime: ServiceStateRuntime::Active,
                    persistent: ServiceStatePersistent::Enabled,
                },
            }),
        )?;
    }

    Ok(())
}

fn aur<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
) -> Result<()> {
    plan.add(
        Ref("creating build user"),
        ctx,
        Box::new(User {
            name: "makepkg",
            home: Ref(Path::new("/var/lib/makepkg")),
            primary_group: PrimaryGroupConfig::Automatic,
            additional_groups: &[],
            shell: "/bin/bash",
            system: true,
        }),
    )?;

    Ok(())
}

fn boot_target<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
    config: &'ctx Config,
) -> Result<()> {
    plan.add(
        Ref("setting boot target"),
        ctx,
        Box::new(BootTarget {
            target: &config.machine.boot_target,
        }),
    )?;

    Ok(())
}

fn sudo<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
) -> Result<()> {
    plan.add(
        Ref("add sudonopw"),
        ctx,
        Box::new(Group {
            name: "sudonopw",
            system: true,
        }),
    )?;

    plan.add(
        Ref("configure passwordless sudo"),
        ctx,
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
            content: Lines(Owned(vec![Ref("%sudonopw ALL=(ALL) NOPASSWD: ALL")])),
        }),
    )?;

    plan.add(
        Ref("configure passwordless doas"),
        ctx,
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
            content: Lines(Owned(vec![Ref("permit nopass nolog setenv {PATH=/usr/local/bin:/usr/local/sbin:/usr/bin:/usr/sbin} :sudonopw")])),
        }),
    )?;

    Ok(())
}

fn timesyncd<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
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
        ctx,
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
            content: Lines(Owned(vec![
                Ref("[Time]"),
                Owned(format!("NTP={}", arch_ntp_servers.join(" "))),
                Owned(format!("FallbackNTP={}", fallback_ntp_servers.join(" "))),
            ])),
        }),
    )?;

    Ok(())
}

fn reflector<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
) -> Result<()> {
    plan.add(
        "configure reflector",
        ctx,
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
            content: Lines(Ref(&[
                Ref("--save /etc/pacman.d/mirrorlist"),
                Ref("--protocol https"),
                Ref("--country Germany"),
                Ref("--latest 5"),
                Ref("--sort age"),
            ])),
        }),
    )?
    .on_change(|plan| {
        plan.add(
            "restart reflector",
            ctx,
            Box::new(RestartService {
                name: "reflector.service",
            }),
        )?;
        Ok(())
    })?;

    plan.add(
        "enable reflector timer",
        ctx,
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

fn pacman_configuration<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
) -> Result<()> {
    plan.add(
        "configure pacman",
        ctx,
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
    .on_change(|plan| {
        plan.add(
            "refresh package database",
            ctx,
            Box::new(PackageDatabaseRefresh {}),
        )?;
        Ok(())
    })?;

    Ok(())
}

fn pacman_cache_cleanup<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
) -> Result<()> {
    plan.add(
        "install pacman-contrib",
        ctx,
        Box::new(Packages {
            install: Ref(&["pacman-contrib"]),
        }),
    )?;
    plan.add(
        "pacman cache cleanup service",
        ctx,
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
        ctx,
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
        ctx,
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

fn install_packages<'name, 'plan, 'ctx, 'action>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
    packages: &'action PackageConfig,
) -> Result<()>
where
    'action: 'plan + 'ctx,
{
    let mut changes = false;

    for &(section, packages) in packages.0 {
        let action = plan.add(
            Owned(format!("install packages for {section}")),
            ctx,
            Box::new(Packages {
                install: Ref(packages),
            }),
        )?;

        if action.changed() {
            changes = true;
        }
    }

    if changes {
        plan.add(
            "restart service manager due to new packages",
            ctx,
            Box::new(DaemonReload {}),
        )?;
    }

    Ok(())
}

fn hardware<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
    config: &'ctx Config,
) -> Result<()> {
    match config.machine.gpu {
        Gpu::Amd => {
            plan.add(
                "install AMDGPU packages",
                ctx,
                Box::new(Packages {
                    install: Ref(&[
                        "mesa",
                        "mesa-utils",
                        "lib32-mesa",
                        "vulkan-radeon",
                        "lib32-vulkan-radeon",
                        "vulkan-headers",
                        "vulkan-tools",
                    ]),
                }),
            )?;

            plan.add(
                "set AMDGPU options",
                ctx,
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
            plan.add(
                "install nouveau packages",
                ctx,
                Box::new(Packages {
                    install: Ref(&[
                        "mesa",
                        "mesa-utils",
                        "lib32-mesa",
                        "vulkan-nouveau",
                        "lib32-vulkan-nouveau",
                        "vulkan-headers",
                        "vulkan-tools",
                    ]),
                }),
            )?;
        }
        Gpu::Intel => {
            plan.add(
                "install intel packages",
                ctx,
                Box::new(Packages {
                    install: Ref(&[
                        "mesa",
                        "mesa-utils",
                        "lib32-mesa",
                        "vulkan-intel",
                        "lib32-vulkan-intel",
                    ]),
                }),
            )?;
        }
    }

    match config.machine.cpu {
        Cpu::Amd => {
            plan.add(
                "install AMD cpu packages",
                ctx,
                Box::new(Packages {
                    install: Ref(&["amd-ucode"]),
                }),
            )?;
        }
        Cpu::Intel => {
            plan.add(
                "install intel cpu packages",
                ctx,
                Box::new(Packages {
                    install: Ref(&["intel-ucode"]),
                }),
            )?;
        }
    }

    Ok(())
}

fn backlight<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
) -> Result<()> {
    plan.add(
        // See https://wiki.archlinux.org/title/backlight#ACPI
        Ref("create udev rule to allow video group backlight access"),
        ctx,
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
            content: Lines(Ref(&[Ref(r#"ACTION=="add", SUBSYSTEM=="backlight", RUN+="/bin/chgrp video $sys$devpath/brightness", RUN+="/bin/chmod g+w $sys$devpath/brightness""#)])),
        }),
    )?;

    Ok(())
}

fn kernel_modules<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
) -> Result<()> {
    plan.add(
        "disable kernel modules",
        ctx,
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

fn journald<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
) -> Result<()> {
    plan.add(
        "limit journald size",
        ctx,
        Box::new(LineInFile {
            path: Path::new("/etc/systemd/journald.conf"),
            line: Ref("SystemMaxUse=50M"),
            regex: Regex::new(r#"^#?SystemMaxUse=.*$"#).expect("regex to be valid"),
        }),
    )?
    .on_change(|plan| {
        plan.add(
            "restart journald",
            ctx,
            Box::new(RestartService {
                name: "systemd-journald.service",
            }),
        )?;
        Ok(())
    })?;
    Ok(())
}

fn hardware_switches<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
) -> Result<()> {
    plan.add(
        "handle lid switch",
        ctx,
        Box::new(LineInFile {
            path: Path::new("/etc/systemd/logind.conf"),
            line: Ref("HandleLidSwitch=ignore"),
            regex: Regex::new(r#"^#?HandleLidSwitch=.*$"#).expect("regex to be valid"),
        }),
    )?;
    plan.add(
        "handle power key",
        ctx,
        Box::new(LineInFile {
            path: Path::new("/etc/systemd/logind.conf"),
            line: Ref("HandlePowerKey=suspend"),
            regex: Regex::new(r#"^#?HandlePowerKey=.*$"#).expect("regex to be valid"),
        }),
    )?;
    Ok(())
}

fn firefox<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
) -> Result<()> {
    for directory in [
        Path::new("/etc/firefox"),
        Path::new("/etc/firefox/policies"),
    ] {
        plan.add(
            Owned(format!("create firefox directories {directory}")),
            ctx,
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
        ctx,
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

fn root_setup<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    ctx: &'ctx Context,
    config: &'ctx Config,
    packages: &'ctx PackageConfig,
) -> Result<(), anyhow::Error> {
    pacman_configuration(plan, ctx)?;
    pacman_cache_cleanup(plan, ctx)?;
    install_packages(plan, ctx, packages)?;
    reflector(plan, ctx)?;
    initrd(plan, ctx, config)?;
    libvirt(plan, ctx)?;
    services(plan, ctx)?;
    boot_target(plan, ctx, config)?;
    aur(plan, ctx)?;
    timesyncd(plan, ctx)?;
    sudo(plan, ctx)?;
    hardware(plan, ctx, config)?;
    hardware_switches(plan, ctx)?;
    journald(plan, ctx)?;
    kernel_modules(plan, ctx)?;
    backlight(plan, ctx)?;
    firefox(plan, ctx)?;
    Ok(())
}

fn configure_user<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    root: &'ctx Context,
    user_ctx: &'ctx Context,
    user: &'ctx UserConfig,
    machine: &'ctx MachineConfig,
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

    user_create_directories(plan, user_ctx, user, &user_directories)?;
    user_executables(plan, user_ctx, user, &user_directories)?;
    user_dotfiles(plan, user_ctx, user, machine, &home)?;
    user_wallpaper(plan, user_ctx, user, &user_directories)?;
    user_services(plan, user_ctx, user, &user_directories, machine)?;
    user_spotify(plan, user_ctx, user, &user_directories)?;
    user_firefox(plan, user_ctx, user, &home)?;

    plan.add(
        "import my gpg key",
        user_ctx,
        Box::new(GpgKey {
            content: include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../gpgkeys/hannes@hkoerber.de.gpg.asc"
            )),
            gnupg_directory: Owned(home.join(".local/state/gnupg/")),
            fingerprint: GpgFingerprint("973AE48D71B76735C4712B5BB5C002530C6A2053".to_owned()),
            trust: GpgTrust::Ultimately,
        }),
    )?;
    Ok(())
}

fn user_firefox<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    user_ctx: &'ctx Context,
    user: &'ctx UserConfig,
    home: &Path,
) -> Result<()> {
    let mozilla_dir = home.join(".mozilla");
    let firefox_dir = mozilla_dir.join("firefox");

    plan.add(
        Ref("create mozilla base directory"),
        user_ctx,
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
        user_ctx,
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
            user_ctx,
            Box::new(Directory {
                path: Owned(profile_dir.clone()),
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
            user_ctx,
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

        plan.add(
            Owned(format!(
                "create firefox userChrome.css for {}",
                profile.name
            )),
            user_ctx,
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

    Ok(())
}

fn user_create_directories<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    user_ctx: &'ctx Context,
    user: &'ctx UserConfig,
    user_directories: &UserDirectories,
) -> Result<()> {
    plan.add(
        "create user config directory",
        user_ctx,
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
        user_ctx,
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
        user_ctx,
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
        user_ctx,
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
        user_ctx,
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
        user_ctx,
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
        user_ctx,
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
        user_ctx,
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
        user_ctx,
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

fn user_spotify<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    user_ctx: &'ctx Context,
    user: &'ctx UserConfig,
    user_directories: &UserDirectories,
) -> Result<()> {
    plan.add(
        Owned("fix spotify audio api".to_owned()),
        user_ctx,
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
            content: FileContent::Lines(Ref(&[Ref("--audio-api=pulseaudio")])),
        }),
    )?;
    Ok(())
}

fn user_autologin<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    root: &'ctx Context,
    user: &'ctx UserConfig,
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

fn user_dotfiles<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    user_ctx: &'ctx Context,
    user: &'ctx UserConfig,
    machine: &'ctx MachineConfig,
    home: &Path,
) -> Result<()> {
    #[derive(Serialize)]
    struct TemplateContext<'a> {
        user: &'a UserConfig,
        machine: &'a MachineConfig,
    }

    for (directory, mode) in [
        (".config/rofi", None),
        (".config/gtk-3.0", None),
        (".config/gtk-3.0-overrides", None),
        (".config/gtk-3.0-overrides/bigger-font/", None),
        (".config/gtk-3.0-overrides/bigger-font/gtk-3.0", None),
        (".config/qt5ct", None),
        (".config/i3status-rust", None),
        (".config/i3status-rust/icons", None),
        (".config/i3status-rust/themes", None),
        (".config/helix", None),
        (".config/git", None),
        (".config/tmux", None),
        (".config/sway", None),
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
            user_ctx,
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
            user_ctx,
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
                "/../applications/firefox.desktop"
            ))
            .as_slice(),
        ),
        (
            ".config/alacritty/themes/monokai.toml",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../alacritty/themes/monokai.toml"
            ))
            .as_slice(),
        ),
        (
            ".config/alacritty/themes/github_light.toml",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../alacritty/themes/github_light.toml"
            ))
            .as_slice(),
        ),
        (
            ".local/state/cargo/config.toml",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../cargo/config.toml")).as_slice(),
        ),
        (
            ".local/state/gnupg/dirmngr.conf",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../gnupg/dirmngr.conf"
            ))
            .as_slice(),
        ),
        (
            ".local/state/gnupg/scdaemon.conf",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../gnupg/scdaemon.conf"
            ))
            .as_slice(),
        ),
        (
            ".config/i3status-rust/config.toml",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../i3status-rust/config.toml"
            ))
            .as_slice(),
        ),
        (
            ".config/i3status-rust/themes/native.toml",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../i3status-rust/themes/native.toml"
            ))
            .as_slice(),
        ),
        (
            ".config/i3status-rust/icons/awesome.toml",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../i3status-rust/icons/awesome.toml"
            ))
            .as_slice(),
        ),
        (
            ".config/tmux/tmux.conf",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../tmux/tmux.conf")).as_slice(),
        ),
        (
            ".zshenv",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../zsh/zshenv")).as_slice(),
        ),
        (
            ".config/gammastep.conf",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../gammastep/gammastep.conf"
            ))
            .as_slice(),
        ),
        (
            ".config/VSCodium/product.json",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../vscodium/product.json"
            ))
            .as_slice(),
        ),
        (
            ".config/VSCodium/User/settings.json",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../vscodium/settings.json"
            ))
            .as_slice(),
        ),
        (
            ".config/VSCodium/User/keybindings.json",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../vscodium/keybindings.json"
            ))
            .as_slice(),
        ),
        (
            ".config/rofi/config",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../rofi/config")).as_slice(),
        ),
        (
            ".config/qt5ct/qt5ct.conf",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../qt/qt5ct.conf")).as_slice(),
        ),
        (
            ".config/helix/config.toml",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../helix/config.toml")).as_slice(),
        ),
        (
            ".config/helix/languages.toml",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../helix/languages.toml"
            ))
            .as_slice(),
        ),
        (
            ".local/state/cargo/config.toml",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../cargo/config.toml")).as_slice(),
        ),
    ] {
        plan.add(
            Owned(format!("{user}: add dotfile ~/{path}", user = user.name)),
            user_ctx,
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
                content: FileContent::Bytes(Ref(content)),
            }),
        )?;
    }

    let template_context = TemplateContext { user, machine };

    let template_context = tera::Context::from_serialize(&template_context)?;

    for (path, template) in [
        (
            ".config/git/config",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../git/gitconfig.j2")),
        ),
        (
            ".local/state/gnupg/gpg-agent.conf",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../gnupg/gpg-agent.conf.j2"
            )),
        ),
        (
            ".local/state/gnupg/gpg.conf",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../gnupg/gpg.conf.j2")),
        ),
        (
            ".config/zsh/.zprofile",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../zsh/zprofile.j2")),
        ),
        (
            ".config/zsh/.zshrc",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../zsh/zshrc.j2")),
        ),
        (
            ".config/dunstrc",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../dunst/dunstrc.j2")),
        ),
        (
            ".config/alacritty/config.toml",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../alacritty/alacritty.toml.j2"
            )),
        ),
        (
            ".config/gtk-3.0/settings.ini",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../gtk/gtk-3.0.ini.j2"
            )),
        ),
        (
            ".config/gtk-3.0-overrides/bigger-font/gtk-3.0/settings.ini",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../gtk/gtk-3.0-bigger-font.ini.j2"
            )),
        ),
        (
            ".config/gtkrc-2.0",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../gtk/gtkrc-2.0.j2")),
        ),
        (
            ".config/sway/config",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sway/config.j2")),
        ),
    ] {
        let template = tera::Tera::one_off(template, &template_context, false)?;
        plan.add(
            Owned(format!("{user}: add dotfile ~/{path}", user = user.name)),
            user_ctx,
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
                content: FileContent::Bytes(Owned(template.into_bytes())),
            }),
        )?;
    }

    Ok(())
}

fn user_executables<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    user_ctx: &'ctx Context,
    user: &'ctx UserConfig,
    user_directories: &UserDirectories,
) -> Result<(), anyhow::Error> {
    for (script, content) in [
        (
            "fix-aoe2.sh",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../scripts/fix-aoe2.sh"
            ))
            .as_slice(),
        ),
        (
            "garmin.sh",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/garmin.sh")).as_slice(),
        ),
        (
            "install-gothic.sh",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../scripts/install-gothic.sh"
            ))
            .as_slice(),
        ),
        (
            "install-gothic2.sh",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../scripts/install-gothic2.sh"
            ))
            .as_slice(),
        ),
    ] {
        plan.add(
            Owned(format!("add script {script}")),
            user_ctx,
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
                content: FileContent::Bytes(Ref(content)),
            }),
        )?;
    }

    for (bin, content) in [
        (
            "ascii",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../bin/ascii")).as_slice(),
        ),
        (
            "asciihex",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../bin/asciihex")).as_slice(),
        ),
        (
            "days-until",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../bin/days-until")).as_slice(),
        ),
        (
            "firefox-default",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../bin/firefox-default"
            ))
            .as_slice(),
        ),
        (
            "git-rewrite-author",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../bin/git-rewrite-author"
            ))
            .as_slice(),
        ),
        (
            "gpg-yubikey-rescan",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../bin/gpg-yubikey-rescan"
            ))
            .as_slice(),
        ),
        (
            "kw",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../bin/kw")).as_slice(),
        ),
        (
            "markdown-to-pdf",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../bin/markdown-to-pdf"
            ))
            .as_slice(),
        ),
        (
            "mousespam",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../bin/mousespam")).as_slice(),
        ),
        (
            "mousespam-toggle",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../bin/mousespam-toggle"
            ))
            .as_slice(),
        ),
        (
            "nato",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../bin/nato")).as_slice(),
        ),
        (
            "natode",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../bin/natode")).as_slice(),
        ),
        (
            "notify",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../bin/notify")).as_slice(),
        ),
        (
            "pacman-remove-orphans",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../bin/pacman-remove-orphans"
            ))
            .as_slice(),
        ),
        (
            "paselect",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../bin/paselect")).as_slice(),
        ),
        (
            "pass",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../bin/pass")).as_slice(),
        ),
        (
            "pf",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../bin/pf")).as_slice(),
        ),
        (
            "pprint",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../bin/pprint")).as_slice(),
        ),
        (
            "pw",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../bin/pw")).as_slice(),
        ),
        (
            "qr",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../bin/qr")).as_slice(),
        ),
        (
            "randmac",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../bin/randmac")).as_slice(),
        ),
        (
            "split-multiyaml-to-json",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../bin/split-multiyaml-to-json"
            ))
            .as_slice(),
        ),
        (
            "switch-color-mode",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../bin/switch-color-mode"
            ))
            .as_slice(),
        ),
        (
            "terraform-get-targets",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../bin/terraform-get-targets"
            ))
            .as_slice(),
        ),
        (
            "unascii",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../bin/unascii")).as_slice(),
        ),
        (
            "yaml2json",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../bin/yaml2json")).as_slice(),
        ),
    ] {
        plan.add(
            Owned(format!("add executable {bin}")),
            user_ctx,
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
                content: FileContent::Bytes(Ref(content)),
            }),
        )?;
    }

    Ok(())
}

fn user_wallpaper<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    user_ctx: &'ctx Context,
    user: &'ctx UserConfig,
    user_directories: &UserDirectories,
) -> Result<()> {
    plan.add(
        Ref("install wallpaper"),
        user_ctx,
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
            #[expect(
                clippy::large_include_file,
                reason = "I want everything in a single binary"
            )]
            content: FileContent::Bytes(Ref(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../wallpaper.jpg"
            )))),
        }),
    )?;

    Ok(())
}
fn user_services<'name: 'plan, 'plan, 'ctx>(
    plan: &'plan mut Plan<'name, 'ctx>,
    user_ctx: &'ctx Context,
    user: &'ctx UserConfig,
    user_directories: &UserDirectories,
    machine: &'ctx MachineConfig,
) -> Result<()> {
    for (unit, content, enabled) in [
        (
            "dunst.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../autostart/services/dunst.service"
            ))
            .as_slice(),
            true,
        ),
        (
            "firefox-gtk-override-bigger-font@.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../autostart/services/firefox-gtk-override-bigger-font@.service"
            ))
            .as_slice(),
            machine.machine_type == MachineType::Tv,
        ),
        (
            "firefox@.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../autostart/services/firefox@.service"
            ))
            .as_slice(),
            true,
        ),
        (
            "gammastep.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../autostart/services/gammastep.service"
            ))
            .as_slice(),
            true,
        ),
        (
            "gnome-keyring.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../autostart/services/gnome-keyring.service"
            ))
            .as_slice(),
            true,
        ),
        (
            "gpg-agent.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../autostart/services/gpg-agent.service"
            ))
            .as_slice(),
            user.gpg.as_ref().is_some_and(|gpg| gpg.agent_enabled),
        ),
        (
            "kdeconnect.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../autostart/services/kdeconnect.service"
            ))
            .as_slice(),
            true,
        ),
        (
            "keepassxc.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../autostart/services/keepassxc.service"
            ))
            .as_slice(),
            user.keepassxc.enabled(),
        ),
        (
            "laptop-lid.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../autostart/services/laptop-lid.service"
            ))
            .as_slice(),
            machine.machine_type == Laptop,
        ),
        (
            "nextcloud.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../autostart/services/nextcloud.service"
            ))
            .as_slice(),
            user.nextcloud.enabled(),
        ),
        (
            "nm-sidebar.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../autostart/services/nm-sidebar.service"
            ))
            .as_slice(),
            true,
        ),
        (
            "screencfg.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../autostart/services/screencfg.service"
            ))
            .as_slice(),
            true,
        ),
        (
            "spotify.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../autostart/services/spotify.service"
            ))
            .as_slice(),
            true,
        ),
        (
            "steam.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../autostart/services/steam.service"
            ))
            .as_slice(),
            user.steam.enabled(),
        ),
        (
            "sway.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../autostart/services/sway.service"
            ))
            .as_slice(),
            true,
        ),
        (
            "windowmanager.target",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../autostart/services/windowmanager.target"
            ))
            .as_slice(),
            true,
        ),
        (
            "workstation-mgr.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../autostart/services/workstation-mgr.service"
            ))
            .as_slice(),
            true,
        ),
        (
            "yubikey-touch-detector.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../autostart/services/yubikey-touch-detector.service"
            ))
            .as_slice(),
            user.gpg.as_ref().is_some_and(|gpg| gpg.agent_enabled),
        ),
    ] {
        let path = Owned(user_directories.systemd_user_units.join(unit));

        if enabled {
            plan.add(
                Owned(format!("user autostart service {unit}")),
                user_ctx,
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
                    content: FileContent::Bytes(Ref(content)),
                }),
            )?;
        } else {
            plan.add(
                Owned(format!("user service {unit}")),
                user_ctx,
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
        user_ctx,
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
                    out.push(if profile.bigger_font {
                        Owned(format!(
                            "Wants=firefox-gtk-override-bigger-font@{name}.service",
                            name = profile.name
                        ))
                    } else {
                        Owned(format!("Wants=firefox@{name}.service", name = profile.name))
                    });
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
                "/../services/color-theme-dark.service"
            ))
            .as_slice(),
        ),
        (
            "color-theme-light.service",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../services/color-theme-light.service"
            ))
            .as_slice(),
        ),
    ] {
        plan.add(
            Owned(format!("user service {unit}")),
            user_ctx,
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
use MachineType::{Laptop, Workstation};

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
    bigger_font: bool,
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

struct Config<'ctx> {
    machine: MachineConfig,
    users: Vec<(Context<'ctx>, UserConfig)>,
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

fn private_user(autologin: bool, vt: usize, name: String) -> UserConfig {
    UserConfig {
        autologin,
        vt,
        name,
        fullname: "Hannes Körber".to_owned(),
        mail: "hannes@hkoerber.de".to_owned(),
        keyboard: UserKeyboardConfig {
            repeat_delay: 150,
            repeat_rate: 50,
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
        steam: FeatureState::Enabled,
        keepassxc: FeatureState::Disabled,
        firefox: FirefoxUserConfig {
            profiles: vec![FirefoxProfile {
                name: "default".to_owned(),
                manage_css: true,
                bigger_font: false,
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

    let root_elevate = match running_user.as_str() {
        "root" => Elevate::No,
        _ => Elevate::Sudo(ElevateTo::Root),
    };

    let initial_state = InitialState::load(&root_elevate)?;

    let ctx = BaseContext {
        initial_state: &initial_state,
    };

    let root = ctx.with_elevate(root_elevate);

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
            profiles: vec![FirefoxProfile {
                name: "default".to_owned(),
                manage_css: true,
                bigger_font: false,
                extensions: vec![
                    "ublock-origin".to_owned(),
                    "passff".to_owned(),
                    "tree-style-tab".to_owned(),
                    "i-dont-care-about-cookies".to_owned(),
                    "floccus".to_owned(),
                ],
            }],
        },
    };

    let config = Config {
        machine: match args.hostname {
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
        },
        users: match args.hostname {
            Neptune => vec![
                private_user(true, 2, "hannes-private".to_owned()),
                work_user,
            ],
            Ares | Dionysus => vec![private_user(true, 1, "hannes".to_owned())],
            Hera => vec![private_user(false, 1, "hannes".to_owned())],
        }
        .into_iter()
        .map(|user| {
            (
                ctx.with_elevate(match running_user.as_str() {
                    "root" => Elevate::No,
                    name if name == user.name => Elevate::No,
                    other => Elevate::Sudo(ElevateTo::User {
                        user: user.name.clone(),
                    }),
                }),
                user,
            )
        })
        .collect(),
    };

    let mut plan = Plan::new();

    root_setup(&mut plan, &root, &config, &PACKAGES)?;

    for user in &config.users {
        let (ref user_ctx, ref user) = *user;

        configure_user(&mut plan, &root, user_ctx, user, &config.machine)?;
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

    let plan_finished = plan.finish();

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
