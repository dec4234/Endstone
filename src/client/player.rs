use std::sync::mpsc::Sender;
use mcproto_rs::types::VarInt;
use mcproto_rs::uuid::UUID4;
use crate::MinecraftClient;
use crate::server::server::PacketBox;

pub struct Player {
    pub id: UUID4,
    pub name: String,
    pub client: MinecraftClient,
    pub sender: Sender<PacketBox>,
}

impl Player {
    pub fn new(client: MinecraftClient, name: &String, sender: Sender<PacketBox>) -> Self {
        Self {
            id: UUID4::random(),
            name: name.clone(),
            client,
            sender,
        }
    }
}

pub struct ClientSettings {
    pub locale: String,
    pub view_distance: u8,
    pub chat_mode: VarInt,
    pub chat_colors: u8,
    pub main_hand: VarInt,
    pub enable_text_filtering: bool,
    pub allow_server_listings: bool,
}