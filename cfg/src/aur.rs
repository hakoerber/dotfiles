#![expect(
    clippy::large_include_file,
    reason = "Locally built packages are part of the binary on purpose"
)]

use maybeowned::prelude::*;
use oxidizer::prelude::*;

use crate::secret::Secrets;

static INCLUDED_PACKAGES: &[(&str, &str, &str, &[u8])] =
    include!(concat!(env!("OUT_DIR"), "/packages.rs"));

pub fn aur<'elevate: 'action, 'action>(
    plan: &mut Block<'_, 'elevate, '_, 'action, '_, Secrets>,
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
