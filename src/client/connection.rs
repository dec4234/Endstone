use std::net::TcpStream;
use craftio_rs::{CraftIo, CraftSyncReader, CraftTcpConnection, CraftWrapper};
use mcproto_rs::protocol::{Packet, PacketDirection, RawPacket};
use anyhow::Result;

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

    pub fn read_next_packet<'a, P: Packet, R: RawPacket<'a, Packet = P>>(&'a mut self) -> Result<Option<P>> {
        if let Some(raw) = self.connection.read_packet::<R>()? {
            Ok(Some(raw))
        } else {
            Ok(None)
        }
    }


}