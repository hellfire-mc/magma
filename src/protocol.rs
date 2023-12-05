//! Defines a variety of protocol-related types.

/// The protocol state.
#[derive(Clone, Default, Debug)]
pub enum ProtocolState {
    /// The protocol is awaiting a handshake.
    #[default]
    Handshaking,
    /// The protocol is awaiting a status request.
    Status,
    /// Login state - the protocol is awaiting a login.
    Login,
    /// Play state - the protocol is connected to a server.
    Play,
}

impl TryFrom<i32> for ProtocolState {
    type Error = anyhow::Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ProtocolState::Handshaking),
            1 => Ok(ProtocolState::Status),
            2 => Ok(ProtocolState::Login),
            3 => Ok(ProtocolState::Play),
            _ => Err(anyhow::anyhow!("Invalid protocol state {}", value)),
        }
    }
}

impl From<&ProtocolState> for i32 {
    fn from(protocol_state: &ProtocolState) -> Self {
        match protocol_state {
            ProtocolState::Handshaking => 0,
            ProtocolState::Status => 1,
            ProtocolState::Login => 2,
            ProtocolState::Play => 3,
        }
    }
}

impl From<ProtocolState> for i32 {
    fn from(protocol_state: ProtocolState) -> Self {
        (&protocol_state).into()
    }
}
