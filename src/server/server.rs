use std::borrow::BorrowMut;
use std::fmt::Debug;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::sync::{Arc, mpsc};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration};
use tokio::sync::Mutex;
use anyhow::{anyhow, Result};
use log::{info, log, warn};
use mcproto_rs::protocol::{RawPacket as RawPacketT, State};
use mcproto_rs::{v1_16_3::Packet753 as Packet, v1_16_3::RawPacket753 as RawPacket};
use mcproto_rs::types::VarInt;
use mcproto_rs::uuid::UUID4;
use mcproto_rs::v1_15_2::Packet578Kind::PlayEditBook;
use mcproto_rs::v1_16_3::{HandshakeNextState, LoginSetCompressionSpec, Packet753, StatusPongSpec};
use tokio::time::Instant;
use crate::{MinecraftClient, ServerStatus};
use crate::client::player::Player;

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
                packet_channel: mpsc::channel(),
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
                    let uuid = UUID4::random();

                    let mut lock = slock.lock().await;

                    let client = MinecraftClient::from_stream(conn.0, lock.packet_channel.0.clone(), uuid);
                    let mut clock = client.lock().await;

                    if let Ok(handshake) = clock.handshake() {
                        if handshake == State::Status {
                            if let Ok(_) = lock.status.send_status(&mut clock) {
                                info!("Send status to: {}", conn.1);
                            } else {
                                warn!("Failed to send status to: {}", conn.1);
                            }
                        } else if handshake == State::Login {
                            // If everything goes okay, add connection to list
                            // tick loop doesn't start soon enough

                            MinecraftClient::broadcast_packets(client.clone()).unwrap();

                            let packet = clock.read_next_packet();

                            drop(clock);

                            if let Ok(Some(packet)) = packet {
                                let r = lock.handle_login(client.clone(), packet).await;

                                if let Ok(player) = r {
                                    // ADD player to list of connections inside server


                                    info!("Established Connection with {} under name: \"{}\"", conn.1, player.name);
                                } else {
                                    warn!("Failed to establish connection with {} - {}", conn.1, r.err().unwrap());
                                }
                            }

                            // println!("{:?}", clock.read_next_packet().unwrap().unwrap()); // Read missed Login Start Packet

                            lock.conns.push((client.clone(), uuid));

                            info!("Established Connection with {}", conn.1);
                        }
                    } else {
                        warn!("Failed to handshake with: {}", conn.1);
                    }
                }
            }
        });

        Server::do_tick_loop(server.clone()).await?;

        Ok(())
    }

    pub async fn do_tick_loop(server: Arc<Mutex<Server>>) -> Result<()> {
        let mut start = Instant::now();

        let server = server.clone();

        loop {
            start = Instant::now();

            Server::handle_packets(server.clone()).await;

            let elapsed = start.elapsed();
            let diff = 50 - elapsed.as_millis();

            if diff > 0 {
                thread::sleep(Duration::from_millis(diff as u64));
            }
        }

        Ok(())
    }

    pub async fn handle_packets(server: Arc<Mutex<Server>>) {
        let server = server.lock().await;

        while let Ok(packet) = server.packet_channel.1.try_recv() {
            if let Some(client) = server.get_client(packet.clientID) {
                let mut c = client.lock().await;

                match packet.packet {

                    Packet::StatusRequest(spec) => {
                        server.status.send_status(&mut c);
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
    }

    pub async fn handle_login(&mut self, client: Arc<Mutex<MinecraftClient>>, packet: Packet) -> Result<Player> {
        let mut name = String::new();

        match packet {
            Packet::LoginStart(spec) => {
                name = spec.name;
            }
            _ => {
                return Err(anyhow!("Found a packet other than LoginStart"));
            }
        }

        let mut clock = client.lock().await;

        clock.write_packet(Packet::LoginSetCompression(LoginSetCompressionSpec {
            threshold: VarInt(256),
        }))?;

        clock.enable_compression();

        Err(anyhow!("temp error"))
    }
}

pub fn convert(handshake: HandshakeNextState) -> State {
    match handshake {
        HandshakeNextState::Status => {
            State::Status
        }
        HandshakeNextState::Login => {
            State::Login
        }
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