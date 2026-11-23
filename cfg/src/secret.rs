use std::io;

use anyhow::Result;
use serde::Deserialize;

use crate::wireguard::WireguardSecrets;

#[derive(Deserialize, Clone)]
pub struct Secret<T: Clone>(T);

impl<T: Clone> std::fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("[redacted]").finish()
    }
}

impl<T: Clone> Secret<T> {
    pub fn access(&self) -> &T {
        &self.0
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Secrets {
    pub wireguard: WireguardSecrets,
}

impl Secrets {
    pub fn load_from(reader: impl io::Read) -> Result<Self> {
        Ok(serde_json::from_reader(reader)?)
    }
}
