use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::TcpListener;

#[path = "client/MinecraftConnection.rs"]
mod MinecraftConnection;

#[tokio::main]
async fn main() {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565);

    let connect = async {
        let mut listener = TcpListener::bind(address).await;
        if let Ok(listener) = &mut listener {
            loop {
                if let Ok((socket, address)) = listener.accept().await {
                    let mut connection = MinecraftConnection::MinecraftConnection::from_tcp_stream(socket);
                    if let Ok(packet) = connection.read_next_packet().await {
                        if let Some(packet) = packet {
                            println!("{:?}", packet);
                        }
                    }
                }
            }
        }
    };

    connect.await
}
