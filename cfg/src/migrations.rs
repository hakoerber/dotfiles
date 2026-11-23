use std::fmt;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use oxidizer::prelude::*;

const STATE_PATH: &str = "/var/lib/cfg.state";

#[derive(Serialize, Deserialize, Debug)]
pub struct MachineState {
    pub migration: Migration,
}

const LATEST_MIGRATION: usize = 4;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Migration {
    id: usize,
}

pub struct MigrationIter {
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
    pub fn apply(&self, elevate: &Elevate) -> Result<()> {
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

    pub fn unapplied(&self) -> MigrationIter {
        MigrationIter {
            migration: self.clone(),
        }
    }

    pub fn latest() -> Self {
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
    pub async fn load(elevate: &Elevate) -> Result<Option<Self>> {
        Ok(try_read(elevate, Path::new(STATE_PATH))
            .await?
            .map(|file| serde_json::from_slice(&file))
            .transpose()?)
    }

    pub async fn save_migration(&mut self, elevate: &Elevate, migration: Migration) -> Result<()> {
        self.migration = migration;
        self.save(elevate).await
    }

    pub async fn save(&self, elevate: &Elevate) -> Result<()> {
        let data = serde_json::to_string(self)?;
        create_or_write(Path::new(STATE_PATH), data.as_bytes(), elevate).await?;
        Ok(())
    }

    pub fn new(migration: Migration) -> Self {
        Self { migration }
    }
}

impl fmt::Display for MachineState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "State({})", self.migration)
    }
}
