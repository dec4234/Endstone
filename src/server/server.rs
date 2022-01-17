use std::net::SocketAddr;
use std::sync::Arc;
use std::thread::Thread;
use std::thread;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::task;
use crate::MinecraftConnection;
use crate::server::player::Player;
use anyhow::Result;
use mcproto_rs::uuid::UUID4;

pub struct Server {
    pub address: SocketAddr,
    pub connected_players: Arc<Mutex<Vec<Player>>>,
    pub listener: TcpListener,
}

impl Server {
    pub async fn new(address: SocketAddr) -> Result<Self> {
        Ok(Self {
            address,
            connected_players: Arc::new(Mutex::new(vec![])),
            listener: TcpListener::bind(address).await?,
        })
    }

    pub fn start(&mut self) {

    }

    pub fn stop(&mut self) {

    }

    fn start_accepting_connections(&'static mut self, mut listener: TcpListener) {
        // self.thread = Some();

        thread::spawn(move || {
            let connect = async {
                loop {
                    if let Ok((socket, address)) = listener.accept().await {
                        let mut connection = MinecraftConnection::from_tcp_stream(socket);
                        self.connected_players.lock().await.push(Player::from_connection(UUID4::random(), String::from("dec4234"), connection));
                    }
                }
            };
        });
    }
}

