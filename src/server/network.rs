use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::{Arc};
use mcproto_rs::status::{StatusFaviconSpec, StatusPlayersSpec, StatusSpec, StatusVersionSpec};
use mcproto_rs::types::{Chat, CountedArray, VarInt};
use mcproto_rs::uuid::UUID4;
use mctokio::{Bridge, TcpConnection, TcpReadBridge, TcpWriteBridge};
use tokio::runtime::Runtime;
use tokio::sync::{Mutex, MutexGuard};
use anyhow::{anyhow, Result};
use mcproto_rs::protocol::State;
use mcproto_rs::protocol::State::Play;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::Receiver;
use mcproto_rs::{v1_16_3 as proto};
use mcproto_rs::v1_16_3::{ChatPosition, HandshakeNextState, LoginEncryptionRequestSpec, LoginSetCompressionSpec, LoginSuccessSpec, Packet753 as Packet, Packet753, PreviousGameMode, RawPacket753 as RawPacket, StatusResponseSpec};
use crate::server::player::Player;

pub struct Client {
    reader: Arc<Mutex<TcpReadBridge>>,
    writer: Arc<Mutex<TcpWriteBridge>>,
}

impl Client {
    pub fn new(stream: TcpConnection) -> Self {
        Self {
            reader: Arc::new(Mutex::new(stream.reader)),
            writer: Arc::new(Mutex::new(stream.writer)),
        }
    }

    pub fn from_tcp_stream(connection: TcpStream) -> Self {
        Self::new(TcpConnection::from_client_connection(connection))
    }

    pub async fn write_packet(&mut self, packet: Packet) -> Result<()> {
        println!("Server -> Client: {:?}", packet);
        self.writer.lock().await.write_packet(packet).await
    }

    pub async fn read_next_packet(&mut self) -> Result<Option<Packet>> {
        if let Some(raw) = self.reader.lock().await.read_packet::<RawPacket>().await? {
            let packet = mcproto_rs::protocol::RawPacket::deserialize(&raw)?;
            println!("Client -> Server: {:?}", packet);
            Ok(Some(packet))
        } else {
            Ok(None)
        }
    }

    pub async fn handshake(&mut self) -> Result<State> {
        let first = self.read_next_packet().await;
        if let Ok(first) = first {
            if let Some(Packet::Handshake(body)) = first {
                match body.next_state {
                    HandshakeNextState::Status => {
                        self.set_state(State::Status).await;
                        return Ok(State::Status);
                    }
                    HandshakeNextState::Login => {
                        self.set_state(State::Login).await;
                        return Ok(State::Login);
                    }
                }
            } else {
                return Err(anyhow!("Did not receive handshake"));
            }
        } else {
            return Err(first.err().unwrap());
        }
    }

    pub async fn set_state(&mut self, state: State) {
        self.reader.lock().await.set_state(state.clone());
        self.writer.lock().await.set_state(state);
    }

    pub async fn set_compression_threshold(&mut self, threshold: i32) {
        self.reader.lock().await.set_compression_threshold(Some(threshold));
        self.writer.lock().await.set_compression_threshold(Some(threshold));
    }

    pub async fn enable_encryption(&mut self, key: &[u8], iv: &[u8]) -> Result<()> {
        let reader = self.reader.lock().await.enable_encryption(key, iv);

        if let Err(error) = reader {
            Err(error)
        } else {
            self.writer.lock().await.enable_encryption(key, iv)
        }
    }
}

pub type NameUUID = (String, UUID4);
pub type ConnectedClients = Arc<Mutex<HashMap<Arc<NameUUID>, Arc<Mutex<ServerClient>>>>>;

pub struct Server {
    clients: ConnectedClients,
    address: SocketAddr,
    online: bool,
    status: ServerStatus,
    entity_ids: Arc<Mutex<HashSet<i32>>>,
    hardcore: bool,
}

impl Server {
    pub fn new(address: SocketAddr, description: &str, max_players: i32, online: bool) -> Self {
        let status = ServerStatus {
            description: Chat::from_traditional(description, true),
            players: StatusPlayersSpec {
                max: max_players,
                online: 0,
                sample: vec![],
            },
            version: StatusVersionSpec {
                name: "Endstone 1.16.2".to_string(),
                protocol: 753,
            },
            favicon: None,
        };

        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            address,
            status,
            online,
            entity_ids: Arc::new(Mutex::new(HashSet::new())),
            hardcore: false,
        }
    }

    pub async fn start(self_mutex: Arc<Mutex<Self>>, mut receiver: Receiver<()>, runtime: Arc<Mutex<Runtime>>) -> Result<()> {
        let mut listener: TcpListener;
        let connections: ConnectedClients;
        {
            let self_lock = self_mutex.lock().await;
            let bind = TcpListener::bind(self_lock.address.clone()).await;
            if let Ok(bind) = bind {
                listener = bind;
                connections = self_lock.clients.clone();
            } else {
                println!("{}", bind.err().unwrap());
                return Err(anyhow!("Failed to bind to {}", self_lock.address.clone()));
            }
        }

        loop {
            if let Err(err) = receiver.try_recv() {
                if err == tokio::sync::mpsc::error::TryRecvError::Closed {
                    break;
                }

                if let Ok((socket, address)) = listener.accept().await {
                    let self_join_arc = self_mutex.clone();
                    let connections = connections.clone();
                    let runtime_arc = runtime.clone();

                    let join = async move {
                        let mut client = Client::from_tcp_stream(socket);
                        let handshake = client.handshake().await;
                        if let Ok(result) = handshake {
                            println!(
                                "{} handshake with {} successful.",
                                result.name(),
                                address.to_string()
                            );
                            if result == mcproto_rs::protocol::State::Login {
                                let login;
                                {
                                    let mut self_lock = self_join_arc.lock().await;
                                    if connections.clone().lock().await.len()
                                        >= self_lock.status.players.max.try_into().unwrap()
                                    {
                                        let _kick = Self::login_kick(
                                            client,
                                            Chat::from_text(
                                                "Server is full, wait for another player to leave.",
                                            ),
                                        )
                                            .await;
                                        return;
                                    }
                                    login = self_lock.handle_login(&mut client, 256).await;
                                }
                                if let Ok(login) = login {
                                    let mut entity_id = i32::MIN;
                                    {
                                        let mut self_lock = self_join_arc.lock().await;
                                        while self_lock.entity_ids.lock().await.contains(&entity_id) {
                                            entity_id = entity_id + 1;
                                        }
                                        self_lock.entity_ids.lock().await.insert(entity_id);
                                    }
                                    let server_client = Arc::new(Mutex::new(ServerClient {
                                        name: login.0.clone(),
                                        uuid: login.1.clone(),
                                        entity_id,
                                        player: Player::new(
                                            login.0.clone(),
                                            login.1.clone(),
                                            entity_id,
                                        ),
                                        connection: client,
                                        view_distance: 10,
                                    }));
                                    {
                                        for player in connections.lock().await.keys() {
                                            if player.0 == login.0 || player.1 == login.1 {
                                                let _kick = server_client.lock().await.kick(Chat::from_text("Someone with the same name or UUID as you is already connected.")).await;
                                                return;
                                            }
                                        }
                                    }
                                    {
                                        let self_lock = self_join_arc.lock().await;
                                        if let Err(_) = server_client.lock().await.join_world(self_lock.hardcore, self_lock.status.players.max).await {
                                            return;
                                        }
                                    }
                                    let server_client_arc = server_client.clone();
                                    let self_loop_arc = self_join_arc.clone();
                                    let packet_loop = async move {
                                        let client_arc = server_client_arc.clone();
                                        let server_arc = self_loop_arc.clone();
                                        loop {
                                            let packet_read: Result<Option<Packet>>;
                                            {
                                                packet_read = client_arc
                                                    .lock()
                                                    .await
                                                    .connection
                                                    .read_next_packet()
                                                    .await;
                                            }
                                            if let Ok(packet_ok) = packet_read {
                                                if let Some(packet) = packet_ok {
                                                    server_arc
                                                        .lock()
                                                        .await
                                                        .handle_packet(
                                                            packet,
                                                            server_client_arc.clone().lock().await,
                                                        ).await;
                                                }
                                            };
                                        }
                                    };
                                    runtime_arc.lock().await.spawn(packet_loop);
                                    connections
                                        .lock()
                                        .await
                                        .insert(Arc::new((login.0, login.1)), server_client);
                                    println!("{} successfully logged in.", address.to_string());
                                } else {
                                    println!(
                                        "{} failed to log in: {}",
                                        address.to_string(),
                                        login.err().unwrap()
                                    )
                                }
                            } else {
                                let status: Result<()>;
                                {
                                    status = self_join_arc.lock().await.handle_status(client).await;
                                }
                                if let Ok(_) = status {
                                    println!(
                                        "{} successfully got server status.",
                                        address.to_string()
                                    )
                                } else {
                                    println!("{} failed to get server status.", address.to_string())
                                }
                            }
                        } else {
                            println!(
                                "Handshake with {} failed: {}",
                                address.to_string(),
                                handshake.err().unwrap()
                            )
                        }
                    };
                    runtime.lock().await.spawn(join);
                }
            }
        }

        Ok(())
    }

    pub async fn handle_login(&mut self, client: &mut Client, compression_threhold: i32) -> Result<(String, UUID4)> {
        use Packet::{LoginEncryptionRequest, LoginEncryptionResponse, LoginSetCompression, LoginStart, LoginSuccess};

        let second = &mut client.read_next_packet().await;
        if let Ok(Some(LoginStart(body))) = second {
            let response = LoginSetCompressionSpec {
                threshold: mcproto_rs::types::VarInt::from(compression_threhold),
            };
            let mut result = (body.name.clone(), UUID4::random());

            if self.online {
                let server_id = "                ".to_string();
                let public_key: &mut [u8] = &mut [0; 16];
                for mut _i in public_key.iter() {
                    _i = &rand::random::<u8>();
                }
                let verify_token: &mut [u8] = &mut [0; 16];
                for mut _i in verify_token.iter() {
                    _i = &rand::random::<u8>();
                }
                let encryption_spec = LoginEncryptionRequestSpec {
                    server_id: server_id.clone(),
                    public_key: CountedArray::from(public_key.to_vec()),
                    verify_token: CountedArray::from(verify_token.to_vec()),
                };
                if let Err(error) = client
                    .write_packet(LoginEncryptionRequest(encryption_spec))
                    .await
                {
                    return Err(error);
                } else {
                    let response = client.read_next_packet().await;
                    if let Ok(Some(LoginEncryptionResponse(response))) = response {
                        if response.verify_token == CountedArray::from(verify_token.to_vec()) {
                            let verify = super::auth::verify_join(
                                &body.name,
                                server_id,
                                &response.shared_secret,
                                public_key,
                            )
                                .await;
                            if let Ok(verified) = verify {
                                if let Err(error) = client.enable_encryption(public_key, verify_token).await {
                                    return Err(error);
                                }

                                result = verified;
                            } else {
                                return Err(verify.err().unwrap());
                            }
                        } else {
                            return Err(anyhow!("Client did not send the correct response to encryption request. {:?}", response));
                        }
                    } else {
                        if let Some(error) = response.err() {
                            return Err(error);
                        } else {
                            return Err(anyhow!("Client did not send a valid response to the encryption request."));
                        }
                    }
                }
            };

            /*
            if let Err(error) = client.write_packet(LoginSetCompression(response)).await {
                return Err(error);
            } else {
                client.set_compression_threshold(compression_threhold).await;
            }
             */

            if let Err(error) = client.write_packet(LoginSuccess(LoginSuccessSpec {
                username: result.0.clone(),
                uuid: result.1.clone(),
            }))
                .await {
                return Err(error);
            }

            client.set_state(Play).await;
            return Ok(result);
        } else {
            return Err(anyhow!("Client did not follow up with Login start"));
        }
    }

    pub async fn login_kick(mut client: Client, message: Chat) -> Result<()> {
        use mcproto_rs::v1_16_3::LoginDisconnectSpec;
        use Packet::LoginDisconnect;

        let spec = LoginDisconnectSpec { message };
        client.write_packet(LoginDisconnect(spec)).await
    }

    #[allow(unused_must_use)]
    pub async fn broadcast_chat(&mut self, message: Chat) {
        for player in self.clients.clone().lock().await.values() {
            player.clone().lock().await.send_message(
                message.clone(),
                ChatPosition::ChatBox,
                None,
            );
        }
    }

    async fn handle_packet(&mut self, packet: Packet, sender: MutexGuard<'_, ServerClient>) {
        match packet {
            Packet::PlayClientChatMessage(body) => {
                self.broadcast_chat(Chat::from_traditional(
                    &("<".to_owned() + sender.name.as_str() + "> " + body.message.as_str()),
                    true,
                ))
                    .await;
            }
            _ => {}
        }
    }

    async fn handle_status(&mut self, mut client: Client) -> anyhow::Result<()> {
        use Packet::{StatusPing, StatusPong, StatusRequest};
        use mcproto_rs::status::StatusPlayerSampleSpec;
        use proto::{StatusPongSpec};
        let second = &mut client.read_next_packet().await;
        if let Ok(second) = second {
            if let Some(StatusRequest(_)) = second {
                {
                    let connected_players = self.clients.lock().await;
                    self.status.players.online = connected_players.len().try_into().unwrap();
                    let mut players: Vec<StatusPlayerSampleSpec> = vec![];
                    for player in connected_players.keys() {
                        players.push(StatusPlayerSampleSpec {
                            id: player.1,
                            name: player.0.clone(),
                        });
                    }
                    self.status.players.sample = players;
                }
                if let Err(error) = self.status.send_status(&mut client).await {
                    return Err(error);
                }
                let third = client.read_next_packet().await;
                if let Ok(third) = third {
                    if let Some(StatusPing(body)) = third {
                        if let Err(error) = client
                            .write_packet(StatusPong(StatusPongSpec {
                                payload: body.payload,
                            }))
                            .await
                        {
                            return Err(error);
                        }
                    }
                }
                return Ok(());
            } else {
                return Err(anyhow::anyhow!(
                    "Client did not send valid packet after login handshake."
                ));
            }
        } else {
            return Err(anyhow::anyhow!(
                "Client did not send valid packet after login handshake."
            ));
        }
    }

}

#[allow(dead_code)]
pub struct ServerClient {
    name: String,
    uuid: UUID4,
    entity_id: i32,
    player: Player,
    view_distance: i32,
    connection: Client,
}

impl ServerClient {
    pub async fn send_message(&mut self, message: Chat, position: ChatPosition, sender: Option<UUID4>) -> Result<()> {
        use Packet::PlayServerChatMessage;
        use proto::PlayServerChatMessageSpec;

        let spec = PlayServerChatMessageSpec {
            message,
            sender: sender.unwrap_or(UUID4::from(0)),
            position,
        };

        let packet = PlayServerChatMessage(spec);
        self.connection.write_packet(packet).await
    }

    pub async fn kick(&mut self, reason: Chat) -> Result<()> {
        use proto::PlayDisconnectSpec;
        use Packet::PlayDisconnect;

        let spec = PlayDisconnectSpec { reason };
        self.connection.write_packet(PlayDisconnect(spec)).await
    }

    pub async fn join_world(&mut self, is_hardcore: bool, max_players: i32) -> Result<()> {
        let spec = proto::PlayJoinGameSpec {
            gamemode: self.player.gamemode.clone(),
            previous_gamemode: PreviousGameMode::NoPrevious,
            entity_id: self.player.entity_id,
            is_hardcore,
            worlds: mcproto_rs::types::CountedArray::from(vec![String::from("world"), String::from("nether")]),
            dimension_codec: mcproto_rs::types::NamedNbtTag { root: mcproto_rs::nbt::NamedTag { name: String::from("oxide"), payload: mcproto_rs::nbt::Tag::End } },
            dimension: mcproto_rs::types::NamedNbtTag { root: mcproto_rs::nbt::NamedTag { name: String::from("oxide"), payload: mcproto_rs::nbt::Tag::End } },
            world_name: String::from("world"),
            hashed_seed: 0,
            max_players: VarInt::from(max_players),
            view_distance: mcproto_rs::types::VarInt::from(self.view_distance),
            enable_respawn_screen: false,
            is_flat: false,
            is_debug: false,
            reduced_debug_info: true,
        };

        self.connection.write_packet(Packet::PlayJoinGame(spec)).await
    }
}

pub struct ServerStatus {
    description: Chat,
    players: StatusPlayersSpec,
    version: StatusVersionSpec,
    favicon: Option<StatusFaviconSpec>,
}

impl ServerStatus {
    pub async fn send_status(&self, client: &mut Client) -> Result<()> {
        let status_spec = StatusSpec {
            description: self.description.clone(),
            favicon: self.favicon.clone(),
            players: self.players.clone(),
            version: Some(self.version.clone()),
        };

        let response_spec = StatusResponseSpec {
            response: status_spec,
        };

        client.write_packet(Packet::StatusResponse(response_spec)).await
    }
}