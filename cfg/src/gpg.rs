use anyhow::bail;

use super::{Elevate, Path, Tempfile, cmd};

use anyhow::{Result, anyhow};

#[derive(Debug, derive_more::Display)]
pub struct GpgFingerprint(pub String);

impl GpgFingerprint {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, PartialEq, Eq, derive_more::Display, Clone, Copy)]
pub enum GpgTrust {
    Unknown,
    Untrusted,
    Marginally,
    Fully,
    Ultimately,
}

impl GpgTrust {
    pub(crate) fn as_str(&self) -> &str {
        match *self {
            Self::Unknown => "2",
            Self::Untrusted => "3",
            Self::Marginally => "4",
            Self::Fully => "5",
            Self::Ultimately => "6",
        }
    }
}

pub(crate) fn get_gpg_key_trust(
    fingerprint: &GpgFingerprint,
    gnupg_directory: &Path,
    elevate: &Elevate,
) -> Result<Option<GpgTrust>> {
    let status_file = Tempfile::new(elevate)?;

    // only interested in the status file
    cmd::Process::run(
        "env",
        &[
            "--ignore-environment",
            &format!("GNUPGHOME={gnupg_directory}"),
            "gpg",
            "--batch",
            "--quiet",
            "--list-public-keys",
            "--keyid-format=long",
            "--status-file",
            status_file.path().as_str(),
            "--with-fingerprint",
            "--with-colons",
            "--",
            &fingerprint.0,
        ],
        elevate,
        None,
    )?
    .unchecked();

    let status = status_file.read_all()?;
    let status = String::from_utf8(status)?;
    let mut status_lines = status.lines();

    // https://github.com/gpg/gnupg/blob/master/doc/DETAILS#format-of-the-status-fd-output
    let key_exists = loop {
        let Some(line) = status_lines.next() else {
            bail!("invalid gpg status, no info");
        };
        let mut segments = line.split_whitespace();

        if segments
            .next()
            .ok_or_else(|| anyhow!("invalid gpg output"))?
            != "[GNUPG:]"
        {
            bail!("invalid gpg status output");
        }

        let keyword = segments
            .next()
            .ok_or_else(|| anyhow!("invalid gpg output"))?;

        match keyword {
            "KEY_CONSIDERED" => break true,
            "ERROR" => {
                segments
                    .next()
                    .ok_or_else(|| anyhow!("invalid gpg output"))?;
                break false;
            }
            // ignore unknown keywords
            _ => (),
        }
    };

    if key_exists {
        let gpg = cmd::Process::run(
            "env",
            &[
                "--ignore-environment",
                &format!("GNUPGHOME={gnupg_directory}"),
                "gpg",
                "--batch",
                "--quiet",
                "--export-ownertrust",
            ],
            elevate,
            None,
        )?
        .check()?;

        for line in gpg.stdout.lines() {
            if line.starts_with('#') {
                continue;
            }

            let [fpt, trustlevel, _comment] = line
                .split(':')
                .collect::<Vec<&str>>()
                .try_into()
                .map_err(|_err| anyhow!("ownertrust field count mismatch"))?;

            if fpt == fingerprint.0 {
                // https://security.stackexchange.com/a/243798
                match trustlevel {
                    "2" => return Ok(Some(GpgTrust::Unknown)),
                    "3" => return Ok(Some(GpgTrust::Untrusted)),
                    "4" => return Ok(Some(GpgTrust::Marginally)),
                    "5" => return Ok(Some(GpgTrust::Fully)),
                    "6" => return Ok(Some(GpgTrust::Ultimately)),
                    other => bail!("unknown trust level {other}"),
                }
            }
        }

        // key not listed in ownertrust, assume unknown trust status
        Ok(Some(GpgTrust::Unknown))
    } else {
        Ok(None)
    }
}

pub(crate) fn trust_key(
    fingerprint: &GpgFingerprint,
    trust: GpgTrust,
    gnupg_directory: &Path,
    elevate: &Elevate,
) -> Result<()> {
    cmd::Process::run(
        "env",
        &[
            "--ignore-environment",
            &format!("GNUPGHOME={gnupg_directory}"),
            "gpg",
            "--batch",
            "--quiet",
            "--import-ownertrust",
        ],
        elevate,
        Some(
            format!(
                // trailing newline is important!
                "{fingerprint}:{trust}:\n",
                fingerprint = fingerprint.as_str(),
                trust = trust.as_str()
            )
            .as_bytes(),
        ),
    )?
    .check()?;

    Ok(())
}

pub(crate) fn import_key(content: &[u8], gnupg_directory: &Path, elevate: &Elevate) -> Result<()> {
    cmd::Process::run(
        "env",
        &[
            "--ignore-environment",
            &format!("GNUPGHOME={gnupg_directory}"),
            "gpg",
            "--batch",
            "--quiet",
            "--import",
        ],
        elevate,
        Some(content),
    )?
    .check()?;

    Ok(())
}
