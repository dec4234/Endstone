use std::borrow::BorrowMut;
use std::fmt::Debug;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::sync::{Arc, mpsc};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration};
use tokio::sync::Mutex;
use anyhow::Result;
use log::{info, log, warn};
use mcproto_rs::protocol::{RawPacket as RawPacketT};
use mcproto_rs::{v1_16_3::Packet753 as Packet, v1_16_3::RawPacket753 as RawPacket};
use mcproto_rs::uuid::UUID4;
use mcproto_rs::v1_16_3::StatusPongSpec;
use tokio::time::Instant;
use crate::{MinecraftClient, ServerStatus};

pub struct Server {
    pub addr: SocketAddr,
    pub status: ServerStatus,
    pub conns: Vec<(Arc<Mutex<MinecraftClient>>, UUID4)>,
    pub packet_channel: (Sender<PacketBox>, Receiver<PacketBox>),
}

impl Server {
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

    pub fn get_client(&self, id: UUID4) -> Option<Arc<Mutex<MinecraftClient>>> {
        for con in self.conns.as_slice() {
            if con.1 == id {
                return Some(con.0.clone());
            }
        }

        None
    }

    /// Handles server start up activities and spawns a thread to process new connections
    pub async fn start(server: Arc<Mutex<Server>>) -> Result<()> {

        let bind = TcpListener::bind(server.lock().await.addr)?;

        let slock = server.clone();

        tokio::spawn(async move {
            loop {
                if let Ok(conn) = bind.accept() {
                    let mut client = MinecraftClient::from_stream(conn.0, slock.lock().await.packet_channel.0.clone());
                    if let Ok(handshake) = client.handshake() {
                        if let Ok(_) = slock.lock().await.status.send_status(&mut client) {
                            // If everything goes okay, add connection to list
                            info!("Established Connection with {}", conn.1);
                            slock.lock().await.conns.push((Arc::new(Mutex::new(client)), UUID4::random()));
                        } else {
                            warn!("Failed to send status to: {}", conn.1);
                        }
                    } else {
                        warn!("Failed to send handshake to: {}", conn.1);
                    }
                }
            }
        });

        Server::do_tick_loop(server.clone()).await?;

        Ok(())
    }

    pub async fn do_tick_loop(server: Arc<Mutex<Server>>) -> Result<()> {
        let mut start= Instant::now();

        let server = server.clone();

        tokio::spawn(async move {
            loop {
                start = Instant::now();
                println!("tick");

                server.lock().await.handle_packets().await;

                let elapsed = start.elapsed();
                let diff = 50 - elapsed.as_millis();

                if diff > 0 {
                    thread::sleep(Duration::from_millis(diff as u64));
                }
            }
        });

        Ok(())
    }

    pub async fn handle_packets(&mut self) {
        /*
            for packet in self.packet_channel.1.iter() {
                if let Some(client) = self.get_client(packet.clientID) {
                    let mut c = client.lock().await;

                    match packet.packet {
                        Packet::StatusRequest(spec) => {
                            self.status.send_status(c.borrow_mut());
                        }
                        Packet::StatusPing(ping) => {
                            c.write_packet(Packet::StatusPong(StatusPongSpec {
                                payload: ping.payload,
                            }));
                        }
                        _ => {}
                    }
                }
            }
         */

        for con in self.conns.as_mut_slice() {
            println!("ran");
            let mut c = con.0.lock().await;
            let packet = c.read_next_packet();
            println!("read");

            if let Ok(Some(packet)) = packet {
                match packet {
                    Packet::StatusRequest(spec) => {
                        self.status.send_status(c.borrow_mut());
                    }
                    Packet::StatusPing(ping) => {
                        c.write_packet(Packet::StatusPong(StatusPongSpec {
                            payload: ping.payload,
                        }));
                    }
                    _ => {}
                }

                break;
            } else {
                break;
            }
        }

        println!("out");
    }
}

pub struct PacketBox {
    pub clientID: UUID4,
    pub packet: Packet,
}

impl PacketBox {
    pub fn new(id: UUID4, packet: Packet) -> Self {
        Self {
            clientID: id,
            packet,
        }
    }
}