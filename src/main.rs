use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc};
use std::time::Duration;
use tokio::net::TcpListener;
use anyhow::Result;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;
use crate::server::network::Server;
use tokio::{sync::{mpsc, Mutex}};
use crate::server::client::Client;

mod server;

#[tokio::main]
async fn main() -> Result<()> {
    // async_main(Arc::new(Mutex::new(Runtime::new()?))).await
    test(Arc::new(Mutex::new(Runtime::new()?))).await
}

async fn test(runtime: Arc<Mutex<Runtime>>,) -> Result<()> {
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565)).await;

    if let Ok(listener) = listener {
        loop {
            let connection = listener.accept().await;

            if let Ok(connection) = connection {
                let (stream, addr) = connection;
                let mut client = Client::from_tcp_stream(stream);
                println!("{:?}", client.read_next_packet().await.unwrap().unwrap());
                println!("{:?}", client.read_next_packet().await.unwrap().unwrap());
            }
        }
    }

    Ok(())
}

async fn async_main(runtime: Arc<Mutex<Runtime>>,) -> Result<()> {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565);

    let server = start_server(address, "&6Endstone".to_string(), false, runtime.clone()).await;
    server.0.await?;
    /*
    let connect = async {
        let mut listener = TcpListener::bind(address).await;
        if let Ok(listener) = &mut listener {
            loop {
                if let Ok((socket, address)) = listener.accept().await {
                    let mut connection = MinecraftConnection::from_tcp_stream(socket);
                    if let Ok(packet) = connection.read_next_packet().await {
                        if let Some(packet) = packet {
                            println!("{:?}", packet);
                        }
                    }
                }
            }
        }
    };

    connect.await;
     */

    Ok(())
}

async fn start_server(address: SocketAddr, description: String, online: bool, runtime: Arc<Mutex<Runtime>>,) -> (JoinHandle<Result<()>>, Arc<Mutex<Server>>, tokio::sync::mpsc::Sender<()>) {
    let server = Arc::new(Mutex::new(Server::new(address, description.as_str(), 5, online)));

    let (tx, rx) = mpsc::channel(20);
    (
        runtime.lock().await.spawn(Server::start(server.clone(), rx, runtime.clone())),
        server,
        tx
        )
}
