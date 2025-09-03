pub mod client;
pub mod server;
pub mod socket;

pub(crate) trait WireCommand {
    fn parse_wire(input: impl Iterator<Item = u8>) -> Result<Self, server::ParseError>
    where
        Self: Sized;

    fn to_wire(&self) -> Vec<u8>;
}
