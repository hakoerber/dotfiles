// required for `serde_json::json!()`
#![recursion_limit = "256"]

pub mod aur;
pub mod hosts;
pub mod language;
pub mod migrations;
pub mod packages;
pub mod root;
pub mod secret;
pub mod sway;
pub mod user;
pub mod wireguard;

mod tree;
