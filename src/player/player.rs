use mcproto_rs::protocol::{PacketDirection};
use mctokio::{TcpConnection, TcpReadBridge, TcpWriteBridge};
use tokio::net::TcpStream;
use mcproto_rs::v1_16_3::{Packet753 as Packet, RawPacket753 as RawPacket};
use anyhow::Result;

pub struct MinecraftConnection {
    reader: TcpReadBridge,
    writer: TcpWriteBridge,
    packet_direction: PacketDirection,
}

impl MinecraftConnection {
    pub fn new(stream: TcpConnection) -> Self {
        Self {
            reader: stream.reader,
            writer: stream.writer,
            packet_direction: PacketDirection::ClientBound,
        }
    }

    pub fn from_tcp_stream(connection: TcpStream) -> Self {
        Self::new(TcpConnection::from_client_connection(connection))
    }

    pub async fn write_packet(&mut self, packet: Packet) {
        self.writer.write_packet(packet).await;
    }

    pub async fn read_next_packet(&mut self) -> Result<Option<Packet>> {
        if let Some(raw) = self.reader.read_packet::<RawPacket>().await? {
            Ok(Some(mcproto_rs::protocol::RawPacket::deserialize(&raw)?))
        } else {
            Ok(None)
        }
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