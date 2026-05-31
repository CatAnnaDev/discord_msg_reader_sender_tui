use std::error::Error;

use futures_util::SinkExt;
use futures_util::stream::SplitSink;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tungstenite::Message;

pub type VoiceSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

#[derive(Deserialize, Debug)]
pub struct VoiceEnvelope {
    pub op: u8,
    #[serde(default)]
    pub d: Value,
    #[serde(default)]
    pub seq: Option<u64>,
}

#[derive(Deserialize, Debug)]
pub struct HelloD {
    pub heartbeat_interval: f64,
}

#[derive(Deserialize, Debug)]
pub struct ReadyD {
    pub ssrc: u32,
    pub ip: String,
    pub port: u16,
    #[serde(default)]
    pub modes: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct SessionDescriptionD {
    pub mode: String,
    pub secret_key: Vec<u8>,
}

#[derive(Serialize, Debug)]
pub struct IdentifyD<'a> {
    pub server_id: &'a str,
    pub user_id: &'a str,
    pub session_id: &'a str,
    pub token: &'a str,
}

pub async fn send_identify(
    sink: &mut VoiceSink,
    server_id: &str,
    user_id: &str,
    session_id: &str,
    token: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let payload = json!({
        "op": 0,
        "d": {
            "server_id": server_id,
            "user_id": user_id,
            "session_id": session_id,
            "token": token,
            "max_dave_protocol_version": crate::voice::dave::MAX_PROTOCOL_VERSION,
        }
    });
    sink.send(Message::text(serde_json::to_string(&payload)?))
        .await?;
    Ok(())
}

pub async fn send_identify_watch(
    sink: &mut VoiceSink,
    server_id: &str,
    user_id: &str,
    session_id: &str,
    token: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let payload = json!({
        "op": 0,
        "d": {
            "server_id": server_id,
            "user_id": user_id,
            "session_id": session_id,
            "token": token,
            "video": true,
            "streams": [
                { "type": "video", "rid": "100", "quality": 100, "active": false }
            ],
            "max_dave_protocol_version": crate::voice::dave::MAX_PROTOCOL_VERSION,
        }
    });
    sink.send(Message::text(serde_json::to_string(&payload)?))
        .await?;
    Ok(())
}

pub async fn send_video_sink_wants(
    sink: &mut VoiceSink,
    audio_ssrc: u32,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let payload = json!({
        "op": 12,
        "d": {
            "audio_ssrc": audio_ssrc,
            "video_ssrc": 0,
            "rtx_ssrc": 0,
            "streams": [
                { "type": "video", "rid": "100", "quality": 100, "active": true }
            ]
        }
    });
    sink.send(Message::text(serde_json::to_string(&payload)?))
        .await?;
    Ok(())
}

pub async fn send_select_protocol(
    sink: &mut VoiceSink,
    address: &str,
    port: u16,
    mode: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let payload = json!({
        "op": 1,
        "d": {
            "protocol": "udp",
            "data": { "address": address, "port": port, "mode": mode },
            "address": address,
            "port": port,
            "mode": mode,
            "codecs": [
                { "name": "opus", "type": "audio", "priority": 1000, "payload_type": 120 },
                { "name": "H264", "type": "video", "priority": 1000, "payload_type": 105,
                  "rtx_payload_type": 106, "encode": true, "decode": true }
            ]
        }
    });
    sink.send(Message::text(serde_json::to_string(&payload)?))
        .await?;
    Ok(())
}

pub async fn send_heartbeat(
    sink: &mut VoiceSink,
    nonce: u64,
    seq_ack: Option<u32>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let payload = match seq_ack {
        Some(s) => json!({ "op": 3, "d": { "t": nonce, "seq_ack": s } }),
        None => json!({ "op": 3, "d": { "t": nonce } }),
    };
    sink.send(Message::text(serde_json::to_string(&payload)?))
        .await?;
    Ok(())
}

pub async fn send_ready_for_transition(
    sink: &mut VoiceSink,
    transition_id: u16,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let payload = json!({ "op": 23, "d": { "transition_id": transition_id } });
    sink.send(Message::text(serde_json::to_string(&payload)?))
        .await?;
    Ok(())
}

pub async fn send_speaking(
    sink: &mut VoiceSink,
    speaking: u8,
    ssrc: u32,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let payload = json!({
        "op": 5,
        "d": {
            "speaking": speaking,
            "delay": 0,
            "ssrc": ssrc,
        }
    });
    sink.send(Message::text(serde_json::to_string(&payload)?))
        .await?;
    Ok(())
}
