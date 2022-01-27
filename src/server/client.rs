use std::sync::Arc;
use mcproto_rs::{v1_16_3 as proto, v1_16_3::Packet753 as Packet, v1_16_3::RawPacket753 as RawPacket};
use mctokio::{Bridge, TcpConnection, TcpReadBridge, TcpWriteBridge};
use tokio::sync::Mutex;
use anyhow::{anyhow, Result};
use mcproto_rs::protocol::State;
use mcproto_rs::v1_16_3::HandshakeNextState;
use tokio::net::TcpStream;

pub struct Client {
    reader: Arc<Mutex<TcpReadBridge>>,
    writer: Arc<Mutex<TcpWriteBridge>>,
}

impl Client {
    pub fn new(stream: TcpConnection) -> Self {
        Self {
            reader: Arc::new(Mutex::new(stream.reader)),
            writer: Arc::new(Mutex::new(stream.writer)),
        }
    }

    pub fn from_tcp_stream(connection: TcpStream) -> Self {
        Self::new(TcpConnection::from_client_connection(connection))
    }

    pub async fn write_packet(&mut self, packet: Packet) -> Result<()> {
        println!("Server -> Client: {:?}", packet);
        self.writer.lock().await.write_packet(packet).await
    }

    pub async fn read_next_packet(&mut self) -> Result<Option<Packet>> {
        if let Some(raw) = self.reader.lock().await.read_packet::<RawPacket>().await? {
            let packet = mcproto_rs::protocol::RawPacket::deserialize(&raw)?;
            println!("Client -> Server: {:?}", packet);
            Ok(Some(packet))
        } else {
            Ok(None)
        }
    }

    pub async fn handshake(&mut self) -> Result<State> {
        let first = self.read_next_packet().await;
        if let Ok(first) = first {
            if let Some(Packet::Handshake(body)) = first {
                match body.next_state {
                    HandshakeNextState::Status => {
                        self.set_state(State::Status).await;
                        return Ok(State::Status);
                    }
                    HandshakeNextState::Login => {
                        self.set_state(State::Login).await;
                        return Ok(State::Login);
                    }
                }
            } else {
                return Err(anyhow!("Did not receive handshake"));
            }
        } else {
            return Err(first.err().unwrap());
        }
    }

    pub async fn set_state(&mut self, state: State) {
        self.reader.lock().await.set_state(state.clone());
        self.writer.lock().await.set_state(state);
    }

    pub async fn set_compression_threshold(&mut self, threshold: i32) {
        self.reader.lock().await.set_compression_threshold(Some(threshold));
        self.writer.lock().await.set_compression_threshold(Some(threshold));
    }

    pub async fn enable_encryption(&mut self, key: &[u8], iv: &[u8]) -> Result<()> {
        let reader = self.reader.lock().await.enable_encryption(key, iv);

        if let Err(error) = reader {
            Err(error)
        } else {
            self.writer.lock().await.enable_encryption(key, iv)
        }
    }
}