use std::borrow::BorrowMut;
use std::fmt::Debug;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::sync::{Arc, mpsc};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration};
use tokio::sync::Mutex;
use anyhow::Result;
use log::{info, warn};
use mcproto_rs::protocol::{RawPacket};
use tokio::time::Instant;
use crate::{MinecraftClient, ServerStatus};

pub struct Server<R: RawPacket<'static> + Send + Debug> {
    pub addr: SocketAddr,
    pub status: ServerStatus,
    pub conns: Vec<MinecraftClient>,
    pub packet_channel: (Sender<R::Packet>, Receiver<R::Packet>),
}

impl<R: 'static +  RawPacket<'static> + Send + Debug> Server<R> {
    pub fn new(port: u16, status: ServerStatus) -> Arc<Mutex<Self>> {
        Self::new_with_ip(IpAddr::from(Ipv4Addr::LOCALHOST), port, status)
    }

    pub fn new_with_ip(ip: IpAddr, port: u16, status: ServerStatus) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(
            Self {
                addr: SocketAddr::new(ip, port),
                status,
                conns: Vec::new(),
                packet_channel: mpsc::channel()
            }))
    }

    /// Handles server start up activities and spawns a thread to process new connections
    pub async fn start(server: Arc<Mutex<Server<R>>>) -> Result<()> where <R as RawPacket<'static>>::Packet: Send {

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
                            slock.lock().await.conns.push(client);
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

    pub async fn do_tick_loop(server: Arc<Mutex<Server<R>>>) -> Result<()> {

        let mut start= Instant::now();

        let server = server.clone();

        tokio::spawn(async move {
            loop {
                start = Instant::now();



                let elapsed = start.elapsed();
                let diff = 50 - elapsed.as_millis();

                if diff > 0 {
                    thread::sleep(Duration::from_millis(diff as u64));
                }
            }
        });

        Ok(())
    }

    pub async fn handle_packets<'a>(&'static mut self) where R::Packet: Debug + Send {
        for c in self.conns.as_mut_slice() {
            let packet = c.read_next_packet::<R>();

            match packet {

                _ => {}
            }
        }
    }


}