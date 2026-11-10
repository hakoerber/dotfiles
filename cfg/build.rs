#![allow(
    clippy::unwrap_used,
    clippy::missing_assert_message,
    clippy::assert_is_empty,
    reason = "fine in build script"
)]

use std::{
    env, fs,
    io::{BufWriter, Write as _},
    path::{Path, PathBuf},
    process,
};

fn build_package<'a>(
    name: &'a str,
    makepkg_path: &Path,
    preexec: Option<&str>,
    update: bool,
) -> Package<'a> {
    let env = vec![("PKGDEST", "../../build/pkg".to_owned())];

    let package_path = {
        if update {
            // run makepkg just to update the version, so we get the correct path with --packagelist afterwards
            let makepkg_for_version = process::Command::new("makepkg")
                .arg("--noextract")
                .arg("--nobuild")
                .arg("--noprepare")
                .envs(env.clone())
                .current_dir(makepkg_path)
                .spawn()
                .unwrap()
                .wait()
                .unwrap();

            assert!(makepkg_for_version.success());
        }

        let makepkg_packagelist = process::Command::new("env")
            .arg("makepkg")
            .arg("--packagelist")
            .envs(env.clone())
            .current_dir(makepkg_path)
            .output()
            .unwrap();

        assert!(makepkg_packagelist.status.success());
        assert!(makepkg_packagelist.stderr.is_empty());

        let stdout = String::from_utf8(makepkg_packagelist.stdout)
            .unwrap()
            .trim()
            .to_owned();

        let stdout: String = stdout
            .lines()
            .filter(|line| !line.contains("-debug"))
            .collect();

        assert_eq!(stdout.lines().count(), 1);

        let path = PathBuf::from(stdout);

        if !path.exists() {
            let temp_directory = tempfile::TempDir::new().unwrap();

            let gnupg_directory = temp_directory.path().join("gpghome");

            let env = {
                let mut existing = env;
                existing.push(("GNUPGHOME", gnupg_directory.display().to_string()));
                existing
            };

            if let Some(preexec) = preexec {
                process::Command::new("install")
                    .arg("--mode=700")
                    .arg("--directory")
                    .arg(&gnupg_directory)
                    .envs(env.clone())
                    .spawn()
                    .unwrap()
                    .wait()
                    .unwrap();

                process::Command::new("gpg").arg("-k").envs(env.clone());

                process::Command::new("bash")
                    .arg("-c")
                    .arg(preexec)
                    .envs(env.clone())
                    .spawn()
                    .unwrap()
                    .wait()
                    .unwrap();
            }

            let makepkg = process::Command::new("makepkg")
                .arg("--clean")
                .arg("--cleanbuild")
                .current_dir(makepkg_path)
                .envs(env.clone())
                .spawn()
                .unwrap()
                .wait()
                .unwrap();

            assert!(makepkg.success());
        }

        assert!(path.exists(), "{path} not found", path = path.display());

        path
    };

    let package_version = {
        let pacman = process::Command::new("pacman")
            .args([
                "--query",
                "--file",
                "--",
                package_path.as_os_str().to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert!(pacman.stderr.is_empty());
        assert!(pacman.status.success());

        let stdout = String::from_utf8(pacman.stdout).unwrap().trim().to_owned();

        assert_eq!(stdout.lines().count(), 1);

        let [_name, version] = stdout
            .split_whitespace()
            .collect::<Vec<&str>>()
            .try_into()
            .unwrap();

        version.to_owned()
    };

    let package_name = package_path.file_name().unwrap().to_str().unwrap();

    Package {
        app_name: name,
        version: package_version,
        filename: package_name.to_owned(),
        path: package_path,
    }
}

struct Package<'a> {
    app_name: &'a str,
    version: String,
    filename: String,
    path: PathBuf,
}

struct BuildInfo {
    name: &'static str,
    path: &'static str,
    preexec: Option<&'static str>,
    update: bool,
}

const PKGBUILDS: &[BuildInfo] = &[
    BuildInfo {
        name: "workstation-mgr",
        path: "../mgr/pkg",
        preexec: None,
        update: true,
    },
    BuildInfo {
        name: "aws-session-manager-plugin",
        path: "../pkgbuilds/aws-session-manager-plugin",
        preexec: None,
        update: false,
    },
    BuildInfo {
        name: "cargo-lambda-bin",
        path: "../pkgbuilds/cargo-lambda-bin",
        preexec: None,
        update: false,
    },
    BuildInfo {
        name: "claude-code",
        path: "../pkgbuilds/claude-code",
        preexec: None,
        update: false,
    },
    BuildInfo {
        name: "grm",
        path: "../pkgbuilds/grm",
        preexec: None,
        update: false,
    },
    BuildInfo {
        name: "nm-sidebar",
        path: "../pkgbuilds/nm-sidebar",
        preexec: None,
        update: false,
    },
    BuildInfo {
        name: "nodejs-intelephense",
        path: "../pkgbuilds/nodejs-intelephense",
        preexec: None,
        update: false,
    },
    BuildInfo {
        name: "portfolio-performance-bin",
        path: "../pkgbuilds/portfolio-performance-bin",
        preexec: Some(
            "curl -sSf --proto '=https' https://keys.openpgp.org/vks/v1/by-fingerprint/E46E6F8FF02E4C83569084589239277F560C95AC | gpg --import -",
        ),
        update: false,
    },
    BuildInfo {
        name: "screencfg",
        path: "../pkgbuilds/screencfg",
        preexec: None,
        update: false,
    },
    BuildInfo {
        name: "slack-desktop",
        path: "../pkgbuilds/slack-desktop",
        preexec: None,
        update: false,
    },
    BuildInfo {
        name: "spotify",
        path: "../pkgbuilds/spotify",
        preexec: Some(
            "curl -sSf --proto '=https' https://download.spotify.com/debian/pubkey_5384CE82BA52C83A.gpg | gpg --import -",
        ),
        update: false,
    },
    BuildInfo {
        name: "terraform-ls-bin",
        path: "../pkgbuilds/terraform-ls-bin",
        preexec: None,
        update: false,
    },
];

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();

    let mut dest_file = BufWriter::new(
        fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(Path::new(&out_dir).join("packages.rs"))
            .unwrap(),
    );

    write!(dest_file, "&[").unwrap();

    if env::var_os("SKIP_PKGBUILDS").is_none_or(|v| v != "1") {
        for pkgbuild in PKGBUILDS {
            let package = build_package(
                pkgbuild.name,
                Path::new(pkgbuild.path),
                pkgbuild.preexec,
                pkgbuild.update,
            );

            write!(
                dest_file,
                r#"("{name}", "{version}", "{package_filename}", include_bytes!("{package_path}")),"#,
                name = package.app_name,
                version = package.version,
                package_filename = package.filename,
                package_path = package.path.display()
            ).unwrap();
        }
    }

    write!(dest_file, "]").unwrap();

    dest_file.flush().unwrap();

    for pkgbuild in PKGBUILDS {
        println!(
            "cargo::rerun-if-changed={path}/PKGBUILD",
            path = pkgbuild.path
        );
    }
    println!("cargo::rerun-if-changed=../files");
    println!("cargo::rerun-if-changed=../assets");
    println!("cargo::rerun-if-env-changed=SKIP_PKGBUILDS");
}
