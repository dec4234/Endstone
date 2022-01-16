#[path = "client/MinecraftConnection.rs"]
mod MinecraftConnection;
mod client;

fn main() {
    MinecraftConnection::MinecraftConnection::new();
}
