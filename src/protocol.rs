use mc_chat::{ChatComponent, ComponentStyle, TextComponent};
use serde::Serialize;

/// The protocol state.
#[derive(Default, Debug)]
pub enum ProtocolState {
    #[default]
    Handshaking,
    Status,
    Login,
    Play,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    pub version: StatusResponseVersion,
    pub players: StatusResponsePlayers,
    pub description: ChatComponent,
    #[serde(default = "String::new")]
    pub favicon: String,
    pub previews_chat: bool,
    pub enforces_secure_chat: bool,
}

impl StatusResponse {
    pub fn message<S: AsRef<str>>(message: S) -> StatusResponse {
        let message = message.as_ref().to_owned();
        StatusResponse {
            version: StatusResponseVersion {
                name: "Unknown".to_string(),
                protocol: 0,
            },
            players: StatusResponsePlayers {
                max: 0,
                online: 0,
                sample: vec![],
            },
            description: ChatComponent::from_text(message, ComponentStyle::v1_15()),
            favicon: "".to_string(),
            previews_chat: false,
            enforces_secure_chat: false,
        }
    }
}

#[derive(Serialize)]
pub struct StatusResponseVersion {
    pub name: String,
    pub protocol: u16,
}

#[derive(Serialize)]
pub struct StatusResponsePlayers {
    pub max: usize,
    pub online: usize,
    pub sample: Vec<StatusResponsePlayer>,
}

#[derive(Serialize)]
pub struct StatusResponsePlayer {
    pub name: String,
    pub uuid: String,
}
