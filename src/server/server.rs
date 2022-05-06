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
use mcproto_rs::nbt::{NamedTag, Tag};
use mcproto_rs::types::{CountedArray, NamedNbtTag, VarInt};
use mcproto_rs::uuid::UUID4;
use mcproto_rs::v1_15_2::Packet578Kind::PlayEditBook;
use mcproto_rs::v1_16_3::{GameMode, HandshakeNextState, LoginSetCompressionSpec, LoginSuccessSpec, Packet753, PlayJoinGameSpec, PreviousGameMode, StatusPongSpec};
use tokio::time::Instant;
use crate::{MinecraftClient, ServerStatus};
use crate::client::player::Player;

pub struct Server {
    pub addr: SocketAddr,
    pub status: ServerStatus,
    pub players: Vec<(Arc<Mutex<Player>>, UUID4)>,
    pub packet_channel: (Sender<PacketBox>, Receiver<PacketBox>),
    pub settings: ServerSettings,
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
                players: Vec::new(),
                packet_channel: mpsc::channel(),
                settings: ServerSettings {
                    view_distance: 8,
                    max_players: 250,
                },
            }))
    }

    pub fn get_client(&self, id: UUID4) -> Option<Arc<Mutex<Player>>> {
        for con in self.players.as_slice() {
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

                    let mut client = MinecraftClient::from_stream(conn.0, lock.packet_channel.0.clone(), uuid);

                    if let Ok(handshake) = client.handshake() {
                        if handshake == State::Status {
                            if let Ok(_) = lock.status.send_status(&mut client) {
                                info!("Send status to: {}", conn.1);
                            } else {
                                warn!("Failed to send status to: {}", conn.1);
                            }
                        } else if handshake == State::Login {
                            // If everything goes okay, add connection to list
                            // tick loop doesn't start soon enough

                            let packet = client.read_next_packet();

                            if let Ok(Some(packet)) = packet {
                                let r = lock.handle_login(client, packet).await;

                                if let Ok(player) = r {
                                    // ADD player to list of connections inside server
                                    info!("Established Connection with {} under name: \"{}\"", conn.1, player.name.clone());

                                    let c = player.id.clone();

                                    let plock = Arc::new(Mutex::new(player));

                                    MinecraftClient::broadcast_packets(plock.clone()).unwrap();

                                    lock.players.push((plock, c));
                                } else {
                                    warn!("Failed to establish connection with {} - {}", conn.1, r.err().unwrap());
                                }
                            }

                            // println!("{:?}", clock.read_next_packet().unwrap().unwrap()); // Read missed Login Start Packet

                            // lock.conns.push((client.clone(), uuid)); // TO-DO: Move this inside, switch list to player

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
                let mut player = client.lock().await;

                match packet.packet {
                    Packet::StatusRequest(spec) => {
                        server.status.send_status(&mut player.client);
                    }
                    Packet::StatusPing(ping) => {
                        player.client.write_packet(Packet::StatusPong(StatusPongSpec {
                            payload: ping.payload,
                        }));
                    }
                    _ => {}
                }
            }
        }
    }

    pub async fn handle_login(&mut self, mut client: MinecraftClient, packet: Packet) -> Result<Player> {
        let mut name = String::new();

        match packet {
            Packet::LoginStart(spec) => {
                name = spec.name;
            }
            _ => {
                return Err(anyhow!("Found a packet other than LoginStart"));
            }
        }

        client.write_packet(Packet::LoginSetCompression(LoginSetCompressionSpec {
            threshold: VarInt(256),
        }))?;

        client.enable_compression();

        let mut player = Player::new(client, &name, self.packet_channel.0.clone());

        player.client.write_packet(Packet::LoginSuccess(LoginSuccessSpec {
            uuid: player.id,
            username: player.name.clone(),
        }))?;

        player.client.set_state(State::Play);

        player.client.write_packet(Packet::PlayJoinGame(self.settings.get_join_game(GameMode::Adventure, 3)))?;

        Ok(player)
    }
}

pub struct ServerSettings {
    pub max_players: u32,
    pub view_distance: u8,
}

impl ServerSettings {
    pub fn new(max_players: u32, view_distance: u8) -> Self {
        Self {
            max_players,
            view_distance,
        }
    }

    pub fn get_join_game(&self, gamemode: GameMode, entityID: i32) -> PlayJoinGameSpec {
        let tag = Tag::Compound(vec![
            NamedTag {
                name: String::from("piglin_safe"),
                payload: Tag::Byte(1),
            },
            NamedTag {
                name: String::from("natural"),
                payload: Tag::Byte(1),
            },
            NamedTag {
                name: String::from("ambient_light"),
                payload: Tag::Float(1.0),
            },
            NamedTag {
                name: String::from("fixed_time"),
                payload: Tag::Long(1),
            },
            NamedTag {
                name: String::from("infiniburn"),
                payload: Tag::String(String::from("minecraft:infiniburn_overworld")),
            },
            NamedTag {
                name: String::from("respawn_anchor_works"),
                payload: Tag::Byte(0),
            },
            NamedTag {
                name: String::from("has_skylight"),
                payload: Tag::Byte(1),
            },
            NamedTag {
                name: String::from("bed_works"),
                payload: Tag::Byte(1),
            },
            NamedTag {
                name: String::from("effects"),
                payload: Tag::String(String::from("minecraft:overworld")),
            },
            NamedTag {
                name: String::from("has_raids"),
                payload: Tag::Byte(0),
            },
            NamedTag {
                name: String::from("min_y"),
                payload: Tag::Int(0),
            },
            NamedTag {
                name: String::from("height"),
                payload: Tag::Int(256),
            },
            NamedTag {
                name: String::from("logical_height"),
                payload: Tag::Int(256),
            },
            NamedTag {
                name: String::from("coordinate_scale"),
                payload: Tag::Float(1.0),
            },
            NamedTag {
                name: String::from("ultrawarm"),
                payload: Tag::Byte(0),
            },
            NamedTag {
                name: String::from("has_ceiling"),
                payload: Tag::Byte(0),
            },
        ]);

        let dimension_type = NamedTag {
            name: String::from(""),
            payload: Tag::Compound(vec![
                NamedTag {
                    name: String::from("minecraft:dimension_type"),
                    payload: Tag::Compound(vec![
                        NamedTag {
                            name: String::from("type"),
                            payload: Tag::String(String::from("minecraft:dimension_type")),
                        },
                        NamedTag {
                            name: String::from("value"),
                            payload: Tag::List(
                                vec![Tag::Compound(vec![
                                    NamedTag {
                                        name: String::from("name"),
                                        payload: Tag::String(String::from("minecraft:overworld")),
                                    },
                                    NamedTag {
                                        name: String::from("id"),
                                        payload: Tag::Int(0),
                                    },
                                    NamedTag {
                                        name: String::from("element"),
                                        payload: tag,
                                    },
                                ])]
                            ),
                        },
                    ]),
                },
            ]),
        };

        let biome = NamedTag {
            name: String::from(""),
            payload: Tag::Compound(vec![
                NamedTag {
                    name: String::from("precipitation"),
                    payload: Tag::String(String::from("rain")),
                },
                NamedTag {
                    name: String::from("effects"),
                    payload: Tag::String(String::from("minecraft:overworld")),
                },
                NamedTag {
                    name: String::from("depth"),
                    payload: Tag::Float(-1.0),
                },
                NamedTag {
                    name: String::from("temperature"),
                    payload: Tag::Float(0.5),
                },
                NamedTag {
                    name: String::from("scale"),
                    payload: Tag::Float(0.1),
                },
                NamedTag {
                    name: String::from("downfall"),
                    payload: Tag::Float(0.5),
                },
                NamedTag {
                    name: String::from("category"),
                    payload: Tag::String(String::from("none")),
                },
                NamedTag {
                    name: String::from("infiniburn"),
                    payload: Tag::String(String::from("minecraft:infiniburn_overworld")),
                },
                NamedTag {
                    name: String::from("logical_height"),
                    payload: Tag::Int(256),
                },
                NamedTag {
                    name: String::from("ambient_light"),
                    payload: Tag::Float(9.0),
                },
                NamedTag {
                    name: String::from("has_raids"),
                    payload: Tag::Byte(1),
                },
                NamedTag {
                    name: String::from("respawn_anchor_works"),
                    payload: Tag::Byte(0),
                },
                NamedTag {
                    name: String::from("bed_works"),
                    payload: Tag::Byte(1),
                },
                NamedTag {
                    name: String::from("piglin_safe"),
                    payload: Tag::Byte(0),
                },
                NamedTag {
                    name: String::from("coordinate_scale"),
                    payload: Tag::Float(1.0),
                },
                NamedTag {
                    name: String::from("ultrawarm"),
                    payload: Tag::Byte(0),
                },
                NamedTag {
                    name: String::from("has_ceiling"),
                    payload: Tag::Byte(0),
                },
                NamedTag {
                    name: String::from("has_skylight"),
                    payload: Tag::Byte(1),
                },
                NamedTag {
                    name: String::from("natural"),
                    payload: Tag::Byte(1),
                },
            ]),
        };

        let spec = PlayJoinGameSpec {
            gamemode: gamemode,
            previous_gamemode: PreviousGameMode::NoPrevious,
            worlds: CountedArray::from(vec![String::from("world")]),
            dimension_codec: NamedNbtTag {
                root: dimension_type.clone(),
            },
            dimension: NamedNbtTag {
                root: biome.clone(),
            },
            world_name: "world".to_string(),
            hashed_seed: 0,
            max_players: VarInt(self.max_players as i32),
            view_distance: VarInt(self.view_distance as i32),
            reduced_debug_info: false,
            enable_respawn_screen: false,
            is_debug: false,
            entity_id: entityID,
            is_hardcore: false,
            is_flat: false,
        };

        spec
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

