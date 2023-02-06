use mc_chat::ChatComponent;
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
