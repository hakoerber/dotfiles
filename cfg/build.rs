#![allow(
    clippy::unwrap_used,
    clippy::missing_assert_message,
    clippy::assert_is_empty,
    reason = "fine in build script"
)]

use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
};

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();

    let package_path = {
        // run makepkg just to update the version, so we get the correct path with --packagelist afterwards
        let makepkg_for_version = process::Command::new("makepkg")
            .arg("--noextract")
            .arg("--nobuild")
            .arg("--noprepare")
            .current_dir("../mgr/pkg")
            .spawn()
            .unwrap()
            .wait()
            .unwrap();

        assert!(makepkg_for_version.success());

        let makepkg_packagelist = process::Command::new("makepkg")
            .arg("--packagelist")
            .current_dir("../mgr/pkg")
            .output()
            .unwrap();

        assert!(makepkg_packagelist.status.success());
        assert!(makepkg_packagelist.stderr.is_empty());

        let stdout = String::from_utf8(makepkg_packagelist.stdout)
            .unwrap()
            .trim()
            .to_owned();

        assert_eq!(stdout.lines().count(), 1);

        let path = PathBuf::from(stdout);

        if !path.exists() {
            let makepkg = process::Command::new("makepkg")
                .arg("--clean")
                .current_dir("../mgr/pkg")
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

    let dest_path = Path::new(&out_dir).join("packages.rs");
    let package_name = package_path.file_name().unwrap().to_str().unwrap();

    fs::write(
        &dest_path,
        format!(
            r#"
            &[
                ("workstation-mgr", "{version}", "{package_name}", include_bytes!("{package_path}")),
            ]
        "#,
            version = package_version,
            package_name = package_name,
            package_path = package_path.display()
        ),
    )
    .unwrap();

    println!(
        "cargo::rerun-if-changed={path}",
        path = package_path.display()
    );
    println!("cargo::rerun-if-changed=../pkgbuilds");
}
