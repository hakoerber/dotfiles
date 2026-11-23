use std::net::Ipv4Addr;

use serde::Deserialize;

use crate::secret::Secret;

#[derive(Deserialize, Clone)]
pub struct WireguardPubkey(String);

impl WireguardPubkey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize, Clone)]
pub struct WireguardPrivkey(String);

impl WireguardPrivkey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize, Clone)]
pub struct WireguardPsk(String);

impl WireguardPsk {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize, Clone)]
pub struct Domain(String);

impl Domain {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize, Clone)]
pub struct Port(u16);

impl Port {
    pub fn as_inner(&self) -> u16 {
        self.0
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct WireguardSecrets {
    pub address: Secret<Ipv4Addr>,
    pub privkey: Secret<WireguardPrivkey>,
    pub psk: Secret<WireguardPsk>,
    pub server_pubkey: Secret<WireguardPubkey>,
    pub server_ip: Secret<Ipv4Addr>,
    pub server_endpoint_domain: Secret<Domain>,
    pub server_endpoint_port: Secret<Port>,
}
