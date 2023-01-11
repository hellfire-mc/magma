/// The protocol state.
#[derive(Default)]
pub enum ProtocolState {
    #[default]
    Handshaking,
    Status,
    Login,
    Play,
}
