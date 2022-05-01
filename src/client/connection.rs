use std::fmt::Debug;
use std::net::TcpStream;
use craftio_rs::{CraftIo, CraftSyncReader, CraftSyncWriter, CraftTcpConnection, CraftWrapper};
use mcproto_rs::protocol::{Packet as PacketT, PacketDirection, RawPacket as RawPacketT, State};
use anyhow::{anyhow, Result};
use log::info;
use mcproto_rs::status::{StatusFaviconSpec, StatusPlayersSpec, StatusSpec, StatusVersionSpec};
use mcproto_rs::types::Chat;
use mcproto_rs::{v1_16_3 as proto, v1_16_3::Packet753 as Packet, v1_16_3::RawPacket753 as RawPacket};
use mcproto_rs::v1_16_3::{HandshakeNextState, StatusResponseSpec};

pub struct MinecraftClient {
    connection: CraftTcpConnection,
}

impl MinecraftClient {
    fn new(connection: CraftTcpConnection) -> Self {
        Self {
            connection
        }
    }

    pub fn from_stream(stream: TcpStream) -> Self {
        Self {
            connection: CraftTcpConnection::from_std(stream, PacketDirection::ServerBound).unwrap(),
        }
    }

    pub fn write_packet<P>(&mut self, packet: P) -> Result<()> where P: PacketT + Debug {
        info!("Outgoing: {:?}", packet);

        if let Err(t) = self.connection.write_packet(packet) {
            return Err(anyhow!("Failed to send packet!"));
        }

        Ok(())
    }

    pub fn read_next_packet<'a, P: PacketT + Debug, R: RawPacketT<'a, Packet = P>>(&'a mut self) -> Result<Option<P>> {
        if let Some(raw) = self.connection.read_packet::<R>()? {
            info!("Incoming: {:?}", &raw);

            Ok(Some(raw))
        } else {
            Ok(None)
        }
    }

    pub fn set_state(&mut self, state: State) {
        self.connection.set_state(state);
    }

    pub fn handshake(&mut self) -> Result<State> {
        // self.connection.set_compression_threshold(Some(512));

        let first = self.read_next_packet::<Packet, RawPacket>();

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
                return Err(anyhow!("No handshake received"));
            }
        } else {
            return Err(first.err().unwrap());
        }
    }
}

#[derive(Clone)]
pub struct ServerStatus {
    pub description: Chat,
    pub players: StatusPlayersSpec,
    pub version: StatusVersionSpec,
    pub favicon: Option<StatusFaviconSpec>,
}

impl ServerStatus {
    pub fn send_status(&self, client: &mut MinecraftClient) -> Result<()> {
        let clon = StatusSpec {
            description: self.description.clone(),
            favicon: self.favicon.clone(),
            players: self.players.clone(),
            version: Some(self.version.clone()),
        };

        let response = StatusResponseSpec {
            response: clon,
        };

        client.write_packet(Packet::StatusResponse(response))
    }
}