use std::sync::Arc;
use mcproto_rs::{v1_16_3 as proto, v1_16_3::Packet753 as Packet, v1_16_3::RawPacket753 as RawPacket};
use tokio::sync::Mutex;
use anyhow::{anyhow, Result};
use craftio_rs::{CraftAsyncReader, CraftAsyncWriter, CraftConnection, CraftIo, CraftSyncReader, CraftSyncWriter, CraftTcpConnection, CraftTokioConnection};
use mcproto_rs::protocol::{PacketDirection, State};
use mcproto_rs::v1_16_3::HandshakeNextState;
use tokio::io::BufReader;
use tokio::net::TcpStream;

pub struct Client {
    connection: Arc<Mutex<CraftTokioConnection>>,
}

impl Client {
    pub fn from_tcp_stream(connection: TcpStream) -> Self {
        let split = connection.into_split();
        Self {
            connection: Arc::new(Mutex::new(CraftTokioConnection::from_async((BufReader::new(split.0), split.1), PacketDirection::ServerBound))),
        }
    }

    pub async fn write_packet(&mut self, packet: Packet) -> Result<()> {
        println!("Server -> Client: {:?}", packet);
        self.connection.lock().await.write_packet_async(packet).await;
        Ok(())
    }

    pub async fn read_next_packet(&mut self) -> Result<Option<Packet>> {
        if let Some(raw) = self.connection.clone().lock().await.read_raw_packet_async::<RawPacket>().await? {
            println!("Client -> Server: {:?}", &raw);
            Ok(Some(mcproto_rs::protocol::RawPacket::deserialize(&raw)?))
            // Ok(Some(raw))
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
        self.connection.lock().await.set_state(state);
        /*
        self.reader.lock().await.set_state(state.clone());
        self.writer.lock().await.set_state(state);
         */
    }

    pub async fn set_compression_threshold(&mut self, threshold: i32) {
        self.connection.lock().await.set_compression_threshold(Some(threshold));
        self.connection.lock().await.set_compression_threshold(Some(threshold));
    }

    pub async fn enable_encryption(&mut self, key: &[u8], iv: &[u8]) -> Result<()> {
        let reader = self.connection.lock().await.enable_encryption(key, iv);

        if let Err(error) = reader {
            Err(anyhow!("Encryption Error {:?}", error))
        } else {
            self.connection.lock().await.enable_encryption(key, iv);
            Ok(())
        }
    }
}