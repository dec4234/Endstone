use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::sync::{Arc};
use anyhow::Result;
use log4rs::append::console::ConsoleAppender;
use log4rs::append::file::FileAppender;
use log4rs::Config;
use log4rs::config::{Appender, Logger, Root};
use log4rs::encode::pattern::PatternEncoder;
use log::LevelFilter;
use tokio::runtime::Runtime;
use tokio::{sync::{mpsc, Mutex}};
use crate::client::connection::{MinecraftClient, ServerStatus};
use mcproto_rs::{v1_16_3 as proto, v1_16_3::Packet753 as Packet, v1_16_3::RawPacket753 as RawPacket};
use mcproto_rs::status::{StatusPlayersSpec, StatusVersionSpec};
use mcproto_rs::types::Chat;
use mcproto_rs::v1_16_3::StatusPongSpec;
use crate::server::server::Server;

pub mod server;
pub mod client;

#[tokio::main]
async fn main() -> Result<()> {
    // async_main(Arc::new(Mutex::new(Runtime::new()?))).await
    async_main().await
}

async fn async_main() -> Result<()> {
    let stdout = ConsoleAppender::builder().build();

    let requests = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {m}{n}")))
        .build("log/l.log")
        .unwrap();

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .appender(Appender::builder().build("requests", Box::new(requests)))
        .logger(Logger::builder()
            .appender("requests")
            .additive(false)
            .build("app::requests", LevelFilter::Info))
        .build(Root::builder().appender("stdout").build(LevelFilter::Info))
        .unwrap();

    let handle = log4rs::init_config(config).unwrap();

    let status = ServerStatus {
        description: Chat::from_traditional("&c&lThis is a test message\n&d&lTest second line &kjbb", true),
        version: StatusVersionSpec {
            name: String::from("test"),
            protocol: 578,
        },
        players: StatusPlayersSpec {
            max: 7,
            online: 0,
            sample: vec![],
        },
        favicon: None,
    };

    let server = Server::<Packet>::new(25565, status);

    Server::start(server).await.unwrap();

    loop {

    }

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

    let status = ServerStatus {
        description: Chat::from_traditional("&c&lThis is a test message\n&d&lTest second line &kjbb", true),
        version: StatusVersionSpec {
            name: String::from("test"),
            protocol: 578,
        },
        players: StatusPlayersSpec {
            max: 7,
            online: 0,
            sample: vec![],
        },
        favicon: None,
    };

    if let Ok((s, ad)) = bind.accept() {
        let mut client = MinecraftClient::from_stream(s);

        println!("Connection established");

        client.handshake().unwrap();
        status.send_status(&mut client).unwrap();

        loop {
            if let Ok(r) = client.read_next_packet::<Packet, RawPacket>() {
                if let Some(packet) = r {
                    match packet {
                        Packet::StatusPing(pack) => {
                            client.write_packet(Packet::StatusPong(StatusPongSpec {
                                payload: pack.payload,
                            })).unwrap()
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
