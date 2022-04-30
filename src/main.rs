use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::sync::{Arc};
use anyhow::Result;
use tokio::runtime::Runtime;
use tokio::{sync::{mpsc, Mutex}};
use crate::client::connection::MinecraftClient;
use mcproto_rs::{v1_16_3 as proto, v1_16_3::Packet753 as Packet, v1_16_3::RawPacket753 as RawPacket};

pub mod server;
pub mod client;

#[tokio::main]
async fn main() -> Result<()> {
    async_main(Arc::new(Mutex::new(Runtime::new()?))).await
}

async fn async_main(runtime: Arc<Mutex<Runtime>>,) -> Result<()> {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565);

    /*
    let server = start_server(address, "&6Endstone".to_string(), false, runtime.clone()).await;
    server.0.await?;
     */

    testing().await;

    Ok(())
}

/*
async fn start_server(address: SocketAddr, description: String, online: bool, runtime: Arc<Mutex<Runtime>>,) -> (JoinHandle<Result<()>>, Arc<Mutex<Server>>, tokio::sync::mpsc::Sender<()>) {
    let server = Arc::new(Mutex::new(Server::new(address, description.as_str(), 5, online)));

    let (tx, rx) = mpsc::channel(20);
    (runtime.lock().await.spawn(Server::start(server.clone(), rx, runtime.clone())), server, tx)
}
 */

async fn testing() {
    let bind = TcpListener::bind(SocketAddr::new(IpAddr::from(Ipv4Addr::LOCALHOST), 25565)).unwrap();

    if let Ok((s, ad)) = bind.accept() {
        let mut client = MinecraftClient::from_stream(s);

        println!("Connection established");

        loop {
            if let Ok(r) = client.read_next_packet::<Packet, RawPacket>() {
                if let Some(packet) = r {
                    println!("{:?}", packet);
                }
            }
        }
    }
}
