use anyhow::anyhow;
use mcproto_rs::uuid::UUID4;
use anyhow::Result;
use serde::{Deserialize, Serialize};

const HAS_JOINED_SERVER_URL: &str = "https://sessionserver.mojang.com/session/minecraft/hasJoined";
const JOIN_SERVER_URL: &str = "https://sessionserver.mojang.com/session/minecraft/join";
const AUTHENTICATE_URL: &str = "https://authserver.mojang.com/authenticate";
const INVALIDATE_URL: &str = "https://authserver.mojang.com/invalidate";
const VALIDATE_URL: &str = "https://authserver.mojang.com/validate";
const SIGNOUT_URL: &str = "https://authserver.mojang.com/signout";
const REFRESH_URL: &str = "https://authserver.mojang.com/refresh";

pub struct Profile {}


pub async fn verify_join(username: &str, server_id: String, shared_secret: &[u8], public_key: &[u8]) -> Result<(String, UUID4)> {
    let client = reqwest::Client::new();
    let response = client.get(
        (HAS_JOINED_SERVER_URL.to_owned()
            + "?username="
            + username
            + "&serverId="
            + super::hash::calc_hash(&server_id, shared_secret, public_key).as_str())
            .as_str(),
    ).send().await;

    if let Ok(response) = response {
        if let Ok(text) = response.text().await {
            if let Ok(join) = serde_json::from_str::<ServerJoinResponse>(&text) {
                return Ok((join.name, join.id));
            }
            return Err(anyhow!("Bad response!"));
        }
        return Err(anyhow!("Empty Response!"));
    };
    return Err(anyhow!("Failed to send request."));
}

/// Minecraft player ID data.
#[derive(Serialize, Deserialize, Clone)]
pub struct MinecraftProfile {
    /// Username of the player.
    pub name: String,
    /// UUID of the player.
    pub id: UUID4,
}

/// Response to a request to authenticate with Mojang.
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct AuthenticateResponse {
    accessToken: String,
    clientToken: String,
    availableProfiles: Vec<MinecraftProfile>,
    selectedProfile: MinecraftProfile,
}

/// Payload containg a client token and an access token.
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct ClientAccessTokenPayload {
    accessToken: String,
    clientToken: String,
}

/// Response to a request to refresh an access token.
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct RefreshResponse {
    accessToken: String,
    clientToken: String,
    selectedProfile: MinecraftProfile,
}

/// Payload sent to invalidate all access tokens associated with an account.
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct SignoutPayload {
    username: String,
    password: String,
}

/// Agent, game type and the version of the agent.
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct Agent {
    name: String,
    version: i8,
}

/// Request sent to ask for an access token to an account using a username and password.
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct AuthenticateRequest {
    agent: Agent,
    username: String,
    password: String,
}

/// Request sent when joining a server.
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct JoinRequest {
    accessToken: String,
    selectedProfile: String,
    serverId: String,
}

/// Response to a request checking if a plyer is authenticated.
#[derive(Serialize, Deserialize, Clone)]
struct ServerJoinResponse {
    id: UUID4,
    name: String,
}
