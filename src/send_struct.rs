use std::error::Error;

use futures_util::stream::SplitSink;
use futures_util::SinkExt;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tungstenite::Message;

use crate::send_struct::force_join_vocal::ForceJoinD;
use crate::send_struct::login_payload::{ClientState, PayloadLogin, Presence, Properties, D};
use crate::send_struct::request_guild_members::RequestGuildD;

mod force_join_vocal;
pub mod login_payload;
pub mod payload;
pub mod presence;
mod request_guild_members;
pub mod resume;
pub(crate) mod start_video;

type Sink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

async fn send_json<T: serde::Serialize>(
    socket: &mut Sink,
    payload: &T,
) -> Result<(), Box<dyn Error>> {
    let body = serde_json::to_string(payload)?;
    socket.send(Message::text(body)).await?;
    Ok(())
}

pub async fn payload(socket: &mut Sink, token: &str) -> Result<(), Box<dyn Error>> {
    let ms = PayloadLogin {
        op: 2,
        d: D {
            token: token.to_string(),

            properties: Properties::default_mac(),
            presence: Presence {
                activities: vec![],
                status: "invisible".to_string(),
                since: 0,
                afk: false,
            },
            compress: false,
            client_state: ClientState::default(),
        },
    };

    send_json(socket, &ms).await
}

pub async fn _presence_update(socket: &mut Sink) -> Result<(), Box<dyn Error>> {
    let ms = presence::PresenceUpdate {
        op: 3,
        d: presence::D {
            since: 0,
            activities: vec![presence::Struct {
                name: "Custom Status".to_string(),
                r#type: 4,
                state: "FOXY".to_string(),
                id: "custom".to_string(),
                emoji: presence::Emoji {
                    name: "🦀".to_string(),
                },
                created_at: 1707067454519,
            }],
            status: "online".to_string(),
            afk: false,
        },
    };
    send_json(socket, &ms).await
}

pub async fn _force_join_vocal(
    socket: &mut Sink,
    guild_id: &str,
    channel_id: &str,
) -> Result<(), Box<dyn Error>> {
    let join = force_join_vocal::ForceJoinVocal {
        op: 4,
        d: ForceJoinD {
            guild_id: guild_id.to_string(),
            channel_id: channel_id.to_string(),
            self_mute: false,
            self_deaf: false,
        },
    };

    send_json(socket, &join).await
}

pub async fn _join_dm_call(
    socket: &mut Sink,
    channel_id: &str,
) -> Result<(), Box<dyn Error>> {
    let p = serde_json::json!({
        "op": 4,
        "d": {
            "guild_id": serde_json::Value::Null,
            "channel_id": channel_id,
            "self_mute": false,
            "self_deaf": false,
        }
    });
    send_json(socket, &p).await
}

pub async fn _leave_vocal(socket: &mut Sink, guild_id: &str) -> Result<(), Box<dyn Error>> {
    let payload = serde_json::json!({
        "op": 4,
        "d": {
            "guild_id": guild_id,
            "channel_id": serde_json::Value::Null,
            "self_mute": false,
            "self_deaf": false,
        }
    });
    socket
        .send(Message::text(serde_json::to_string(&payload)?))
        .await?;
    Ok(())
}

pub async fn _set_voice_state(
    socket: &mut Sink,
    guild_id: Option<&str>,
    channel_id: &str,
    self_mute: bool,
    self_deaf: bool,
) -> Result<(), Box<dyn Error>> {
    let g = match guild_id {
        Some(g) => serde_json::Value::String(g.to_string()),
        None => serde_json::Value::Null,
    };
    let payload = serde_json::json!({
        "op": 4,
        "d": {
            "guild_id": g,
            "channel_id": channel_id,
            "self_mute": self_mute,
            "self_deaf": self_deaf,
        }
    });
    socket
        .send(Message::text(serde_json::to_string(&payload)?))
        .await?;
    Ok(())
}

pub async fn _watch_stream(
    socket: &mut Sink,
    stream_key: &str,
) -> Result<(), Box<dyn Error>> {
    let payload = serde_json::json!({
        "op": 20,
        "d": { "stream_key": stream_key }
    });
    socket
        .send(Message::text(serde_json::to_string(&payload)?))
        .await?;
    Ok(())
}

pub async fn _create_stream(
    socket: &mut Sink,
    guild_id: Option<&str>,
    channel_id: &str,
) -> Result<(), Box<dyn Error>> {
    let d = match guild_id {
        Some(g) => serde_json::json!({
            "type": "guild",
            "guild_id": g,
            "channel_id": channel_id,
            "preferred_region": serde_json::Value::Null,
        }),
        None => serde_json::json!({
            "type": "call",
            "channel_id": channel_id,
            "preferred_region": serde_json::Value::Null,
        }),
    };
    let payload = serde_json::json!({ "op": 18, "d": d });
    socket
        .send(Message::text(serde_json::to_string(&payload)?))
        .await?;
    Ok(())
}

pub async fn _delete_stream(
    socket: &mut Sink,
    stream_key: &str,
) -> Result<(), Box<dyn Error>> {
    let payload = serde_json::json!({
        "op": 19,
        "d": { "stream_key": stream_key }
    });
    socket
        .send(Message::text(serde_json::to_string(&payload)?))
        .await?;
    Ok(())
}

pub async fn _stream_ping(
    socket: &mut Sink,
    stream_key: &str,
) -> Result<(), Box<dyn Error>> {
    let payload = serde_json::json!({
        "op": 21,
        "d": { "stream_key": stream_key }
    });
    socket
        .send(Message::text(serde_json::to_string(&payload)?))
        .await?;
    Ok(())
}

pub async fn _stop_watching_stream(
    _socket: &mut Sink,
    _stream_key: &str,
) -> Result<(), Box<dyn Error>> {
    Ok(())
}

pub async fn _request_guild_members(
    socket: &mut Sink,
    guild_id: &str,
    query: &str,
    limit: i64,
) -> Result<(), Box<dyn Error>> {
    let req = request_guild_members::RequestGuildMembers {
        op: 8,
        d: RequestGuildD {
            guild_id: guild_id.to_owned(),
            query: query.to_owned(),
            limit,
        },
    };

    send_json(socket, &req).await
}
