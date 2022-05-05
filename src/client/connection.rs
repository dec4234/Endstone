use std::fmt::{Debug, Display};
use std::io::Read;
use std::net::TcpStream;
use std::sync::{Arc};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};
use craftio_rs::{CraftIo, CraftSyncReader, CraftSyncWriter, CraftTcpConnection, CraftWrapper};
use mcproto_rs::protocol::{Packet as PacketT, PacketDirection, RawPacket as RawPacketT, State};
use anyhow::{anyhow, Result};
use log::info;
use mcproto_rs::status::{StatusFaviconSpec, StatusPlayersSpec, StatusSpec, StatusVersionSpec};
use mcproto_rs::types::Chat;
use mcproto_rs::{v1_16_3 as proto, v1_16_3::Packet753 as Packet, v1_16_3::RawPacket753 as RawPacket};
use mcproto_rs::uuid::UUID4;
use mcproto_rs::v1_16_3::{HandshakeNextState, RawPacket753, StatusResponseSpec};
use tokio::sync::Mutex;
use crate::server::server::PacketBox;

pub struct MinecraftClient {
    connection: CraftTcpConnection,
    sender: Sender<PacketBox>,
    pub uuid: UUID4,
    pub previous: Option<Packet>,
}

impl MinecraftClient {
    fn new(connection: CraftTcpConnection, sender: Sender<PacketBox>, uuid: UUID4) -> Self {
        Self {
            connection,
            sender,
            uuid,
            previous: None,
        }
    }

    pub fn from_stream(stream: TcpStream, sender: Sender<PacketBox>, uuid: UUID4) -> Arc<Mutex<Self>> {
        let client = Arc::new(Mutex::new(MinecraftClient::new(CraftTcpConnection::from_std(stream, PacketDirection::ServerBound).unwrap(), sender, uuid)));

        client
    }

    pub fn write_packet(&mut self, packet: Packet) -> Result<()> {
        info!("Outgoing: {:?}", packet);

        if let Err(t) = self.connection.write_packet(packet) {
            return Err(anyhow!("Failed to send packet!"));
        }

        Ok(())
    }

    pub fn read_next_packet(&mut self) -> Result<Option<Packet>> {
        if let Some(raw) = self.connection.read_packet::<RawPacket>()? {
            info!("Incoming: {:?}", &raw);

            self.previous = Some(raw.clone());

            Ok(Some(raw))
        } else {
            Ok(None)
        }
    }

    pub fn broadcast_packets(sel: Arc<Mutex<MinecraftClient>>) -> Result<()> {

        tokio::spawn(async move {
            let sender = sel.lock().await.sender.clone();

            let mut start = Instant::now();

            loop {
                start = Instant::now();

                let mut slock = sel.lock().await;

                let packet = slock.read_next_packet();

                if let Ok(Some(packet)) = packet {
                    sender.send(PacketBox::new(slock.uuid, packet));
                }

                let elapsed = start.elapsed();
                let diff = 10 - elapsed.as_millis();

                if diff > 0 {
                    thread::sleep(Duration::from_millis(diff as u64));
                }
            }
        });

        Ok(())
    }

    pub fn set_state(&mut self, state: State) {
        self.connection.set_state(state);
    }

    pub fn handshake(&mut self) -> Result<State> {
        // self.connection.set_compression_threshold(Some(512));

        let first = self.read_next_packet();

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
            println!("First Error");
            return Err(first.err().unwrap());
        }
    }

    pub fn enable_compression(&mut self) {
        self.connection.set_compression_threshold(Some(256));
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