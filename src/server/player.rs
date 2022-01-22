use std::borrow::Borrow;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Receiver, Sender};
use mcproto_rs::protocol::{PacketDirection};
use mctokio::{TcpConnection, TcpReadBridge, TcpWriteBridge};
use tokio::net::TcpStream;
use mcproto_rs::v1_16_3::{GameMode, Packet753 as Packet, RawPacket753 as RawPacket};
use anyhow::Result;
use mcproto_rs::uuid::UUID4;
use std::thread;
use mcproto_rs::types::{ItemStack, Slot, VarInt};

pub struct Health {
    pub health: i32,
    pub hunger: i32,
    pub saturation: i32,
}

pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub dimension: i32,
    pub world: String,
}

pub struct Player {
    pub name: String,
    pub uuid: UUID4,
    pub entity_id: i32,
    pub position: Position,
    pub health: Health,
    pub inventory: PlayerInventory,
    pub gamemode: GameMode,

}

impl Player {
    pub fn new(name: String, uuid: UUID4, entity_id: i32) -> Self {
        Self {
            name,
            uuid,
            entity_id,
            position: Position {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                dimension: 0,
                world: "world".to_string(),
            },
            health: Health {
                health: 20,
                hunger: 20,
                saturation: 20,
            },
            inventory: PlayerInventory::new_empty(),
            gamemode: GameMode::Spectator,
        }
    }
}

pub struct PlayerInventory {
    pub items: [Slot; 44],
}

impl PlayerInventory {
    pub fn new(items: [Slot; 44]) -> Self {
        Self { items }
    }

    pub fn new_empty() -> Self {
        //         let items: [Option<ItemStack>; 44] = [stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone(), stack.clone()];
        //         let items: [Option<ItemStack>; 44] = [None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None];
        let items: [Option<ItemStack>; 44] = [None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None];
        Self { items }
    }

    pub fn get_armor(&self) -> [Slot; 4] {
        self.items.clone()[5..8]
            .to_owned()
            .try_into()
            .expect("Inventory did not have enough slots.")
    }

    pub fn get_inventory(&self) -> [Slot; 27] {
        self.items.clone()[9..35]
            .to_owned()
            .try_into()
            .expect("Inventory did not have enough slots.")
    }

    pub fn get_hotbar(&self) -> [Slot; 9] {
        self.items.clone()[36..44]
            .to_owned()
            .try_into()
            .expect("Inventory did not have enough slots.")
    }

    pub fn get_offhand(&self) -> Slot {
        self.items[45].clone()
    }

    pub fn get_crafting_input(&self) -> [Slot; 4] {
        self.items.clone()[1..4]
            .to_owned()
            .try_into()
            .expect("Inventory did not have enough slots.")
    }

    pub fn get_crafting_output(&self) -> Slot {
        self.items[0].clone()
    }
}

/*
pub struct MinecraftConnection {
    reader: TcpReadBridge,
    writer: TcpWriteBridge,
    packet_direction: PacketDirection,
}

impl MinecraftConnection {
    pub fn new(connection: TcpConnection, packet_direction: PacketDirection) -> Self {
        Self {
            reader: connection.reader,
            writer: connection.writer,
            packet_direction,
        }
    }

    pub fn from_tcp_stream(connection: TcpStream) -> Self {
        Self::new(TcpConnection::from_client_connection(connection),
                  PacketDirection::ClientBound, )
    }

    pub async fn write_packet(&mut self, packet: Packet) -> Result<()> {
        self.writer.write_packet(packet).await
    }

    pub async fn read_next_packet(&mut self) -> Result<Option<Packet>> {
        if let Some(raw) = self.reader.read_packet::<RawPacket>().await? {
            Ok(Some(mcproto_rs::protocol::RawPacket::deserialize(&raw)?))
        } else {
            Ok(None)
        }
    }

    pub async fn handshake(&mut self, next_state: Option<proto::HandshakeNextState>, name: Option<String>) -> anyhow::Result<State> {
        if self.packet_direction == PacketDirection::ClientBound {
            let first = self.read_next_packet().await;
            if let Ok(first) = first {
                if let Some(Packet::Handshake(body)) = first {
                    match body.next_state {
                        HandshakeNextState::Status => {
                            self.set_state(State::Status);
                            return Ok(State::Status);
                        }
                        HandshakeNextState::Login => {
                            self.set_state(State::Login);
                            return Ok(State::Login);
                        }
                    }
                } else {
                    return Err(anyhow!("Did not receive handshake"));
                }
            } else {
                return Err(first.err().unwrap());
            }
        } else {
            if let Some(next_state) = next_state {
                let handshake = proto::HandshakeSpec {
                    version: mcproto_rs::types::VarInt::from(753),
                    server_address: "".to_string(),
                    server_port: 25565,
                    next_state: next_state.clone(),
                };
                if let Err(error) = self.write_packet(Packet::Handshake(handshake)).await {
                    return Err(error);
                } else {
                    if next_state == proto::HandshakeNextState::Status {
                        self.set_state(State::Status);
                        if let Err(error) = self
                            .write_packet(Packet::StatusRequest(proto::StatusRequestSpec {}))
                            .await
                        {
                            return Err(error);
                        }

                        return Ok(State::Status);
                    } else {
                        self.set_state(State::Login);
                        if let Some(name) = name {
                            if let Err(error) = self
                                .write_packet(Packet::LoginStart(proto::LoginStartSpec {
                                    name: name.clone(),
                                }))
                                .await
                            {
                                return Err(error);
                            }

                            return Ok(State::Login);
                        } else {
                            return Err(anyhow!("Username cannot be empty"));
                        }
                    }
                }
            } else {
                return Err(anyhow!("Cannot handshake as a player without next state"));
            }
        }
    }

    pub fn set_state(&mut self, state: State) {
        self.reader.set_state(state.clone());
        self.writer.set_state(state);
    }
}
 */