use thiserror::Error;

use super::cmd;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Cmd(#[from] cmd::Error),
    #[error("unknown status output: \"{output}\"")]
    UnknownStatusOutput { output: String },
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum UnitStatus {
    Active,
    Inactive,
}

impl UnitStatus {
    pub(crate) fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }
}

pub(crate) mod user {
    use super::{super::cmd, Error, UnitStatus};

    pub(crate) fn unit_status(unit: &str) -> Result<UnitStatus, Error> {
        let output = cmd::run_command("systemctl", &["--user", "is-active", unit])?;
        match output.stdout.as_str().trim() {
            "active" => Ok(UnitStatus::Active),
            "inactive" => Ok(UnitStatus::Inactive),
            other => Err(Error::UnknownStatusOutput {
                output: other.to_owned(),
            }),
        }
    }
}
