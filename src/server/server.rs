use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::sync::{Arc, mpsc};
use std::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use anyhow::Result;
use log::{info, warn};
use mcproto_rs::protocol::Packet;
use crate::{MinecraftClient, ServerStatus};

pub struct Server<P: Packet + Send> {
    pub addr: SocketAddr,
    pub status: ServerStatus,
    pub conns: Arc<Mutex<Vec<MinecraftClient>>>,
    pub packet_channel: (Sender<P>, Receiver<P>),
}

impl<P: 'static +  Packet + Send> Server<P> {
    pub fn new(port: u16, status: ServerStatus) -> Arc<Mutex<Self>> {
        Self::new_with_ip(IpAddr::from(Ipv4Addr::LOCALHOST), port, status)
    }

    pub fn new_with_ip(ip: IpAddr, port: u16, status: ServerStatus) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(
            Self {
                addr: SocketAddr::new(ip, port),
                status,
                conns: Arc::new(Mutex::new(Vec::new())),
                packet_channel: mpsc::channel()
            }))
    }

    /// Handles server start up activities and spawns a thread to process new connections
    pub async fn start(server: Arc<Mutex<Server<P>>>) -> Result<()> {

        let bind = TcpListener::bind(server.lock().await.addr)?;

        let slock = server.clone();

        tokio::spawn(async move {
            loop {
                if let Ok(conn) = bind.accept() {
                    let mut client = MinecraftClient::from_stream(conn.0);
                    if let Ok(handshake) = client.handshake() {
                        if let Ok(_) = slock.lock().await.status.send_status(&mut client) {
                            // If everything goes okay, add connection to list
                            info!("Established Connection with {}", conn.1);
                            slock.lock().await.conns.lock().await.push(client);
                        } else {
                            warn!("Failed to send status to: {}", conn.1);
                        }
                    } else {
                        warn!("Failed to send handshake to: {}", conn.1);
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn do_tick_loop(server: Arc<Mutex<Server<P>>>) -> Result<()> {

        Ok(())
    }
}