/*
use std::io::stdin;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::thread::Thread;
use std::{io, thread};
use std::borrow::{Borrow, BorrowMut};
use std::sync::mpsc::{Receiver, Sender};
use tokio::task;
use crate::MinecraftConnection;
use crate::server::player::Player;
use anyhow::Result;
use mcproto_rs::uuid::UUID4;
use std::time::{SystemTime, UNIX_EPOCH};
use log::log;
use mcproto_rs::v1_16_3::Packet753;
use tokio::net::TcpListener;

pub struct Server {
    pub address: SocketAddr,
    pub connected_players: Arc<Mutex<Vec<MinecraftConnection>>>,
    listener: TcpListener,
    recv: Receiver<Package>,
    pub sender: Mutex<Sender<Package>>,
}

impl Server {
    pub async fn new(address: SocketAddr) -> Result<Self> {
        println!("Server Started!");

        let (send, recv) = std::sync::mpsc::channel();

        Ok(Self {
            address,
            connected_players: Arc::new(Mutex::new(vec![])),
            listener: TcpListener::bind(address).await?,
            recv,
            sender: Mutex::new(send),
        })
    }

    pub async fn start(self, sel: Arc<Mutex<Self>>) {
        self.do_tick_loop(sel.clone()).await.unwrap();
    }

    pub fn stop(&mut self) {}

    async fn do_tick_loop(self, sel: Arc<Mutex<Self>>) -> Result<()> {
        let start = SystemTime::now();
        let mut time = start.duration_since(UNIX_EPOCH).unwrap().as_millis();

        loop {
            let start = SystemTime::now();
            if start.duration_since(UNIX_EPOCH).unwrap().as_millis() > time + 50 {
                time = start.duration_since(UNIX_EPOCH).unwrap().as_millis();

                sel.clone().lock().tick(sel.clone()).await?;
            }
        }
    }

    async fn tick(self, sel: Arc<Mutex<Self>>) -> Result<()> {
        self.process_incoming_packets(sel).await;

        Ok(())
    }

    async fn process_incoming_packets(&self, sel: Arc<Mutex<Self>>) -> Result<()> {
        let mut lis = TcpListener::bind(sel.lock().unwrap().address).await?;
        let connect = MinecraftConnection::from_tcp_stream(lis.accept().await.unwrap().0, sel.lock().unwrap().sender.lock().unwrap().clone());

        sel.lock().unwrap().connected_players.lock().unwrap().push(connect);

        Ok(())
    }
}

pub struct Package {
    packet: Packet753,
    uuid: UUID4,
}

impl Package {
    pub fn new(packet: Packet753, uuid: UUID4) -> Self {
        Self {
            packet,
            uuid,
        }
    }
}
 */

