use std::error::Error;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::select;
use tokio::sync::mpsc;

use crate::config::config::Config;
use crate::discord_connection::{self, GatewaySink, GatewayStream};
use crate::heart_beat::heart_beat;
use crate::receive_struct::gateway::{GatewayEnvelope, opcode};
use crate::receive_struct::hello::HelloData;
use crate::receive_struct::message_create::MessageCreateData;
use crate::receive_struct::message_delete::MessageDeleteData;
use crate::receive_struct::ready::{Guild, ReadyData};
use crate::receive_struct::receive_event::DiscordEventMessage;
use crate::send_struct::{
    _force_join_vocal, _join_dm_call, _leave_vocal, _request_guild_members, _stop_watching_stream,
    _watch_stream, payload,
};
use crate::tui::state::{AppState, ChannelSummary, DisplayMessage, GuildSummary, SharedState};
use crate::utils::SnowflakeID;
use crate::voice::{VoiceCommand, VoiceManager, VoiceSignal};
use crate::{error, info, warn};

const GATEWAY_URL: &str = "wss://gateway.discord.gg/?encoding=json&v=10";
const DM_GUILD_ID: SnowflakeID = SnowflakeID::const_zero();

#[derive(Debug)]
pub enum GatewayAction {
    JoinVoice {
        guild: SnowflakeID,
        channel: SnowflakeID,
    },
    JoinDmCall {
        channel: SnowflakeID,
    },
    LeaveVoice {
        guild: SnowflakeID,
    },
    RequestMembers {
        guild: SnowflakeID,
    },
    WatchStream {
        stream_key: String,
        user: SnowflakeID,
    },
    StopStream,
    StopOneStream {
        uid: u64,
    },
    GoLive,
    StopGoLive,
    SetVoiceState {
        mute: bool,
        deaf: bool,
    },
}

pub async fn run_gateway(
    config: Config,
    state: SharedState,
    mut action_rx: mpsc::UnboundedReceiver<GatewayAction>,
) -> Result<(), Box<dyn Error>> {
    let (mut sink, mut stream) = discord_connection::discord_wss_connection(GATEWAY_URL).await?;
    payload(&mut sink, &config.token).await?;

    let mut hb = tokio::time::interval(Duration::from_millis(41_250));
    hb.tick().await;
    let mut sping = tokio::time::interval(Duration::from_secs(5));
    sping.tick().await;
    crate::info!("=== BUILD MARKER: vtdec-hw + op21-ping + no-decode-resize ===");

    let mut last_seq: Option<u64> = None;
    let mut session_id = String::new();
    let mut resume_gateway = String::new();
    let mut empty_reads = 0;

    loop {
        select! {
            msg = stream.next() => {
                match msg {
                    Some(Ok(m)) => {
                        empty_reads = 0;
                        let Ok(text) = m.to_text() else { continue };
                        let env: GatewayEnvelope = match serde_json::from_str(text) {
                            Ok(e) => e,
                            Err(_) => continue,
                        };
                        if let Some(s) = env.s { last_seq = Some(s); }
                        if let Err(e) = handle_envelope(
                            env,
                            text,
                            &mut sink,
                            &mut stream,
                            &mut hb,
                            &mut last_seq,
                            &mut session_id,
                            &mut resume_gateway,
                            &config,
                            &state,
                        ).await {
                            error!("ingest envelope: {e}");
                        }
                    }
                    Some(Err(e)) => {
                        error!("ws read: {e}");
                        state.write().await.conn_status = "reconnecting…".to_string();
                        reconnect(&mut sink, &mut stream, &config.token, &session_id, &resume_gateway, last_seq).await?;
                    }
                    None => {
                        empty_reads += 1;
                        if empty_reads >= 5 { return Err("5 consecutive empty reads".into()); }
                        state.write().await.conn_status = "reconnecting…".to_string();
                        reconnect(&mut sink, &mut stream, &config.token, &session_id, &resume_gateway, last_seq).await?;
                    }
                }
            }
            _ = hb.tick() => {
                if let Err(e) = heart_beat(&mut sink, last_seq).await {
                    error!("heartbeat: {e}");
                }
            }
            _ = sping.tick() => {
                let keys: Vec<String> = {
                    let s = state.read().await;
                    s.pending_streams.keys().cloned().collect()
                };
                if !keys.is_empty() {
                    crate::info!(
                        "STREAM_PING op21 → {} stream(s): {keys:?}",
                        keys.len()
                    );
                }
                for k in keys {
                    if let Err(e) =
                        crate::send_struct::_stream_ping(&mut sink, &k).await
                    {
                        error!("stream_ping: {e}");
                    }
                }
            }
            action = action_rx.recv() => {
                match action {
                    Some(GatewayAction::JoinVoice { guild, channel }) => {

                        {
                            let mut s = state.write().await;
                            s.joining_voice_token = None;
                            s.joining_voice_endpoint = None;
                            s.joining_voice_session = None;
                            s.joining_voice_guild = Some(guild);
                            s.voice_status = format!("joining voice {channel}…");
                        }
                        if let Err(e) = _force_join_vocal(
                            &mut sink,
                            &guild.to_string(),
                            &channel.to_string(),
                        ).await {
                            error!("force_join_vocal: {e}");
                        }
                    }
                    Some(GatewayAction::JoinDmCall { channel }) => {
                        {
                            let mut s = state.write().await;
                            s.joining_voice_token = None;
                            s.joining_voice_endpoint = None;
                            s.joining_voice_session = None;
                            s.joining_voice_guild = None;
                            s.voice_channel = Some(channel);
                            s.voice_status = format!("calling DM {channel}…");
                        }
                        if let Err(e) =
                            _join_dm_call(&mut sink, &channel.to_string()).await
                        {
                            error!("join_dm_call: {e}");
                        }
                    }
                    Some(GatewayAction::LeaveVoice { guild }) => {
                        if let Err(e) = _leave_vocal(&mut sink, &guild.to_string()).await {
                            error!("leave_vocal: {e}");
                        }
                        let tx = {
                            let mut s = state.write().await;
                            s.voice_channel = None;
                            s.voice_status = "left voice".to_string();
                            s.voice_cmd_tx.take()
                        };
                        if let Some(tx) = tx {
                            let _ = tx.send(VoiceCommand::Disconnect);
                        }
                    }
                    Some(GatewayAction::RequestMembers { guild }) => {
                        if let Err(e) =
                            _request_guild_members(&mut sink, &guild.to_string(), "", 0).await
                        {
                            error!("request_guild_members: {e}");
                        }
                    }
                    Some(GatewayAction::WatchStream { stream_key, user }) => {
                        {
                            let mut s = state.write().await;
                            s.pending_streams
                                .entry(stream_key.clone())
                                .or_default()
                                .uid = user.as_u64();
                            s.voice_status =
                                format!("requesting stream {stream_key}…");
                        }
                        if let Err(e) =
                            _watch_stream(&mut sink, &stream_key).await
                        {
                            error!("watch_stream: {e}");
                        }
                    }
                    Some(GatewayAction::StopStream) => {
                        let (keys, txs) = {
                            let mut s = state.write().await;
                            let keys: Vec<String> =
                                s.pending_streams.keys().cloned().collect();
                            let txs: Vec<_> =
                                s.stream_conns.drain().map(|(_, t)| t).collect();
                            s.pending_streams.clear();
                            (keys, txs)
                        };
                        crate::info!(
                            "GatewayAction::StopStream → stopping {} pending, {} conns",
                            keys.len(),
                            txs.len()
                        );
                        for k in keys {
                            let _ = _stop_watching_stream(&mut sink, &k).await;
                        }
                        for tx in txs {
                            let _ = tx.send(VoiceCommand::Disconnect);
                        }
                    }
                    Some(GatewayAction::StopOneStream { uid }) => {
                        let (key, tx) = {
                            let mut s = state.write().await;
                            let key = s
                                .pending_streams
                                .iter()
                                .find(|(_, p)| p.uid == uid)
                                .map(|(k, _)| k.clone());
                            if let Some(k) = &key {
                                s.pending_streams.remove(k);
                            }
                            (key, s.stream_conns.remove(&uid))
                        };
                        crate::info!(
                            "GatewayAction::StopOneStream uid={uid} key={key:?}"
                        );
                        if let Some(k) = key {
                            let _ = _stop_watching_stream(&mut sink, &k).await;
                        }
                        if let Some(tx) = tx {
                            let _ = tx.send(VoiceCommand::Disconnect);
                        }
                    }
                    Some(GatewayAction::GoLive) => {
                        let (guild, chan, uid) = {
                            let s = state.read().await;
                            (
                                s.joining_voice_guild
                                    .filter(|g| *g != DM_GUILD_ID)
                                    .map(|g| g.to_string()),
                                s.voice_channel.map(|c| c.to_string()),
                                s.my_user_id.map(|u| u.to_string()),
                            )
                        };
                        match (chan, uid) {
                            (Some(c), Some(u)) => {
                                let key = match &guild {
                                    Some(g) => format!("guild:{g}:{c}:{u}"),
                                    None => format!("call:{c}:{u}"),
                                };
                                {
                                    let mut s = state.write().await;
                                    s.golive_key = Some(key.clone());
                                    s.pending_streams
                                        .entry(key.clone())
                                        .or_default()
                                        .uid = u.parse().unwrap_or(0);
                                }
                                crate::info!("GoLive: op18 STREAM_CREATE key={key}");
                                if let Err(e) = crate::send_struct::_create_stream(
                                    &mut sink,
                                    guild.as_deref(),
                                    &c,
                                )
                                .await
                                {
                                    error!("create_stream: {e}");
                                }
                            }
                            _ => {
                                state.write().await.set_status(
                                    "join a voice channel first".to_string(),
                                );
                            }
                        }
                    }
                    Some(GatewayAction::StopGoLive) => {
                        let (key, tx) = {
                            let mut s = state.write().await;
                            let k = s.golive_key.take();
                            if let Some(k) = &k {
                                s.pending_streams.remove(k);
                            }
                            (k, s.golive_cmd_tx.take())
                        };
                        if let Some(k) = key {
                            crate::info!("StopGoLive: op19 STREAM_DELETE key={k}");
                            let _ =
                                crate::send_struct::_delete_stream(&mut sink, &k)
                                    .await;
                        }
                        if let Some(tx) = tx {
                            let _ = tx.send(VoiceCommand::Disconnect);
                        }
                    }
                    Some(GatewayAction::SetVoiceState { mute, deaf }) => {
                        let (guild, chan) = {
                            let s = state.read().await;
                            (
                                s.joining_voice_guild
                                    .filter(|g| *g != DM_GUILD_ID)
                                    .map(|g| g.to_string()),
                                s.voice_channel.map(|c| c.to_string()),
                            )
                        };
                        if let Some(chan) = chan {
                            if let Err(e) = crate::send_struct::_set_voice_state(
                                &mut sink,
                                guild.as_deref(),
                                &chan,
                                mute,
                                deaf,
                            )
                            .await
                            {
                                error!("set_voice_state: {e}");
                            }
                        }
                    }
                    None => {}
                }
            }
        }

        if state.read().await.should_quit {
            return Ok(());
        }
    }
}

async fn reconnect(
    sink: &mut GatewaySink,
    stream: &mut GatewayStream,
    token: &str,
    session_id: &str,
    resume_gateway: &str,
    last_seq: Option<u64>,
) -> Result<(), Box<dyn Error>> {
    use futures_util::SinkExt;
    let _ = sink.close().await;
    let url = if !resume_gateway.is_empty() {
        format!(
            "{}/?encoding=json&v=10",
            resume_gateway.trim_end_matches('/')
        )
    } else {
        GATEWAY_URL.to_string()
    };
    let (new_sink, new_stream) = discord_connection::discord_wss_connection(&url).await?;
    *sink = new_sink;
    *stream = new_stream;
    if !session_id.is_empty() && last_seq.is_some() {
        discord_connection::discord_wss_resume(sink, token, session_id, last_seq.unwrap_or(0))
            .await?;
    } else {
        payload(sink, token).await?;
    }
    Ok(())
}

async fn handle_envelope(
    env: GatewayEnvelope,
    raw: &str,
    sink: &mut GatewaySink,
    stream: &mut GatewayStream,
    hb: &mut tokio::time::Interval,
    last_seq: &mut Option<u64>,
    session_id: &mut String,
    resume_gateway: &mut String,
    config: &Config,
    state: &SharedState,
) -> Result<(), Box<dyn Error>> {
    match env.op {
        opcode::HELLO => {
            if let Ok(h) = serde_json::from_value::<HelloData>(env.d) {
                info!("HELLO heartbeat_interval={}ms", h.heartbeat_interval);
                *hb = tokio::time::interval(Duration::from_millis(h.heartbeat_interval));
                hb.tick().await;
                heart_beat(sink, *last_seq).await?;
            }
        }
        opcode::HEARTBEAT => {
            heart_beat(sink, *last_seq).await?;
        }
        opcode::RECONNECT => {
            warn!("RECONNECT requested");
            reconnect(
                sink,
                stream,
                &config.token,
                session_id,
                resume_gateway,
                *last_seq,
            )
            .await?;
        }
        opcode::INVALID_SESSION => {
            let resumable = env.d.as_bool().unwrap_or(false);
            warn!("INVALID_SESSION resumable={resumable}");
            if resumable && !session_id.is_empty() {
                discord_connection::discord_wss_resume(
                    sink,
                    &config.token,
                    session_id,
                    last_seq.unwrap_or(0),
                )
                .await?;
            } else {
                tokio::time::sleep(Duration::from_secs(2)).await;
                payload(sink, &config.token).await?;
            }
        }
        opcode::DISPATCH => {
            let ev: DiscordEventMessage = match serde_json::from_str(raw) {
                Ok(e) => e,
                Err(_) => return Ok(()),
            };
            apply_event(ev, state, session_id, resume_gateway).await;
        }
        _ => {}
    }
    Ok(())
}

async fn apply_event(
    ev: DiscordEventMessage,
    state: &SharedState,
    session_id: &mut String,
    resume_gateway: &mut String,
) {
    match ev {
        DiscordEventMessage::Ready { d } => {
            if let Some(rg) = &d.resume_gateway_url {
                *resume_gateway = rg.clone();
            }
            if let Some(sid) = &d.session_id {
                *session_id = sid.clone();
            }
            apply_ready(*d, state).await;
            let s = state.clone();
            tokio::spawn(async move { fetch_thumbnails(s).await });
        }
        DiscordEventMessage::MessageCreate { d } | DiscordEventMessage::MessageUpdate { d } => {
            apply_message(d, state).await;
        }
        DiscordEventMessage::MessageDelete { d } => {
            apply_message_delete(d, state).await;
        }
        DiscordEventMessage::VoiceStateUpdate { d } => {
            {
                let uid = d.user_id;
                let mut s = state.write().await;

                if let Some(m) = &d.member {
                    let g = [m.user.global_name.as_str(), m.user.display_name.as_str()]
                        .into_iter()
                        .find(|s| !s.trim().is_empty());
                    s.note_user(uid, g, &m.user.username);
                }
                let name = s.display_name(uid);
                for members in s.voice_members.values_mut() {
                    members.retain(|m| m.id != uid);
                }
                if let Some(ch) = d.channel_id {
                    s.voice_members
                        .entry(ch)
                        .or_default()
                        .push(crate::tui::state::VoiceMember {
                            id: uid,
                            name,
                            mute: d.mute || d.self_mute,
                            deaf: d.deaf || d.self_deaf,
                            video: d.self_video,
                            stream: d.self_stream.unwrap_or(false),
                        });
                }
                s.voice_members.retain(|_, v| !v.is_empty());
            }

            let me = state.read().await.my_user_id;
            if me == Some(d.user_id) {
                if let Some(ch) = d.channel_id {
                    let mut s = state.write().await;
                    s.joining_voice_session = Some(d.session_id.clone());
                    s.voice_channel = Some(ch);
                } else {
                    let mut s = state.write().await;
                    s.voice_channel = None;
                }
                maybe_start_voice(state).await;
            }
        }
        DiscordEventMessage::VoiceServerUpdate { d } => {
            {
                let mut s = state.write().await;
                s.joining_voice_token = Some(d.token.clone());
                s.joining_voice_endpoint = Some(d.endpoint.clone());
                match d.guild_id.as_deref().and_then(|g| g.parse::<u64>().ok()) {
                    Some(g) => {
                        s.joining_voice_guild = Some(SnowflakeID::from_u64(g));
                    }
                    None => {
                        s.joining_voice_guild = s.voice_channel;
                    }
                }
            }
            maybe_start_voice(state).await;
        }
        DiscordEventMessage::StreamCreate { d } => {
            crate::info!("STREAM_CREATE d={d}");
            let key = d.get("stream_key").and_then(|v| v.as_str());
            let srv = d
                .get("rtc_server_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if let Some(key) = key {
                let mut s = state.write().await;
                let e = s.pending_streams.entry(key.to_string()).or_default();
                if srv.is_some() {
                    e.server = srv;
                }
            }
        }
        DiscordEventMessage::StreamServerUpdate { d } => {
            crate::info!("STREAM_SERVER_UPDATE d={d}");
            let key = d
                .get("stream_key")
                .and_then(|v| v.as_str())
                .map(String::from);
            if let Some(key) = key {
                {
                    let mut s = state.write().await;
                    let e = s.pending_streams.entry(key.clone()).or_default();
                    if let Some(t) = d.get("token").and_then(|v| v.as_str()) {
                        e.token = Some(t.to_string());
                    }
                    if let Some(ep) = d.get("endpoint").and_then(|v| v.as_str()) {
                        e.endpoint = Some(ep.to_string());
                    }
                    if e.server.is_none() {
                        if let Some(g) = d.get("guild_id").and_then(|v| v.as_str()) {
                            e.server = Some(g.to_string());
                        }
                    }
                }
                let is_ours = state.read().await.golive_key.as_deref() == Some(&key);
                if is_ours {
                    maybe_start_golive(state, session_id, &key).await;
                } else {
                    maybe_start_stream(state, session_id, &key).await;
                }
            }
        }
        DiscordEventMessage::StreamUpdate { d } => {
            crate::info!("STREAM_UPDATE d={d}");
        }
        DiscordEventMessage::StreamDelete { d } => {
            crate::info!("STREAM_DELETE d={d}");
            let key = d
                .get("stream_key")
                .and_then(|v| v.as_str())
                .map(String::from);
            let tx = {
                let mut s = state.write().await;
                let uid = key
                    .as_ref()
                    .and_then(|k| s.pending_streams.get(k).map(|p| p.uid));
                if let Some(k) = &key {
                    s.pending_streams.remove(k);
                }
                if let Some(uid) = uid {
                    let sid = SnowflakeID::from_u64(uid);
                    s.watching_streams.retain(|x| *x != sid);
                    s.stream_conns.remove(&uid)
                } else {
                    None
                }
            };
            if let Some(tx) = tx {
                let _ = tx.send(VoiceCommand::Disconnect);
            }
        }
        DiscordEventMessage::PresenceUpdate { d } => {
            let mut s = state.write().await;
            s.presence.insert(
                d.user.id,
                crate::tui::state::PresenceStatus::parse(&d.status),
            );
            s.note_user(
                d.user.id,
                d.user.global_name.as_deref(),
                d.user.username.as_deref().unwrap_or(""),
            );
        }
        DiscordEventMessage::TypingStart { d } => {
            let mut s = state.write().await;
            if let Some(m) = &d.member {
                s.note_user(d.user_id, m.user.global_name.as_deref(), &m.user.username);
            }
            let now = std::time::Instant::now();
            let entry = s.typing.entry(d.channel_id).or_default();
            entry.retain(|(u, t)| *u != d.user_id && t.elapsed() < crate::tui::state::TYPING_TTL);
            entry.push((d.user_id, now));
        }
        DiscordEventMessage::MessageReactionAdd { d } => {
            let name = d
                .emoji
                .name
                .clone()
                .or(d.emoji.id.clone())
                .unwrap_or_else(|| "?".into());
            let mut s = state.write().await;
            if let Some(buf) = s.messages.get_mut(&d.channel_id) {
                if let Some(m) = buf.iter_mut().find(|m| m.id == d.message_id) {
                    if let Some(r) = m.reactions.iter_mut().find(|(n, _)| *n == name) {
                        r.1 += 1;
                    } else {
                        m.reactions.push((name, 1));
                    }
                }
            }
        }
        DiscordEventMessage::MessageReactionRemove { d } => {
            let name = d
                .emoji
                .name
                .clone()
                .or(d.emoji.id.clone())
                .unwrap_or_else(|| "?".into());
            let mut s = state.write().await;
            if let Some(buf) = s.messages.get_mut(&d.channel_id) {
                if let Some(m) = buf.iter_mut().find(|m| m.id == d.message_id) {
                    if let Some(p) = m.reactions.iter().position(|(n, _)| *n == name) {
                        if m.reactions[p].1 <= 1 {
                            m.reactions.remove(p);
                        } else {
                            m.reactions[p].1 -= 1;
                        }
                    }
                }
            }
        }
        DiscordEventMessage::ChannelCreate { d } | DiscordEventMessage::ChannelUpdate { d } => {
            let mut s = state.write().await;
            let entry = s.channels_by_guild.entry(d.guild_id).or_default();
            let summary = ChannelSummary {
                id: d.id,
                name: d.name.clone(),
                recipient_id: None,
                kind: d.type_field,
                parent_id: d.parent_id,
                position: d.position,
                last_activity: d.last_message_id.map(u64::from).unwrap_or(0),
                avatar_url: None,
            };
            if let Some(c) = entry.iter_mut().find(|c| c.id == d.id) {
                *c = summary;
            } else {
                entry.push(summary);
                entry
                    .sort_by_key(|c| (c.parent_id.map(u64::from).unwrap_or(0), c.position, c.kind));
            }
        }
        DiscordEventMessage::ChannelDelete { d } => {
            let mut s = state.write().await;
            for chans in s.channels_by_guild.values_mut() {
                chans.retain(|c| c.id != d.id);
            }
        }
        DiscordEventMessage::GuildMemberChunk { d } => {
            let gid = d.guild_id;
            let mut s = state.write().await;
            for m in &d.members {
                let Some(u) = m.get("user") else { continue };
                let Some(uid) = u
                    .get("id")
                    .and_then(|x| x.as_str())
                    .and_then(|x| x.parse::<u64>().ok())
                    .map(SnowflakeID::from_u64)
                else {
                    continue;
                };
                let uname = u.get("username").and_then(|x| x.as_str()).unwrap_or("");
                let gname = u.get("global_name").and_then(|x| x.as_str());
                s.note_user(uid, gname, uname);
                s.guild_members.entry(gid).or_default().insert(uid);
            }
            for p in &d.presences {
                if let Some(uid) = p
                    .get("user")
                    .and_then(|u| u.get("id"))
                    .and_then(|x| x.as_str())
                    .and_then(|x| x.parse::<u64>().ok())
                    .map(SnowflakeID::from_u64)
                {
                    if let Some(st) = p.get("status").and_then(|x| x.as_str()) {
                        s.presence
                            .insert(uid, crate::tui::state::PresenceStatus::parse(st));
                    }
                }
            }
        }
        _ => {}
    }
}

async fn maybe_start_voice(state: &SharedState) {
    let signal = {
        let s = state.read().await;
        match (
            &s.joining_voice_token,
            &s.joining_voice_endpoint,
            &s.joining_voice_session,
            s.joining_voice_guild,
            s.my_user_id,
        ) {
            (Some(token), Some(endpoint), Some(session), Some(guild), Some(uid)) => {
                Some(VoiceSignal {
                    guild_id: guild.to_string(),
                    user_id: uid.to_string(),
                    channel_id: s
                        .voice_channel
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "0".to_string()),
                    session_id: session.clone(),
                    token: token.clone(),
                    endpoint: endpoint.clone(),
                    input_device: s.voice_input_device.clone(),
                    output_device: s.voice_output_device.clone(),
                    dsp_rx: s.prefs.dsp_rx,
                    dsp_tx: s.prefs.dsp_tx,
                    watch_uid: None,
                    server_id_override: None,
                    broadcast: false,
                })
            }
            _ => None,
        }
    };
    let Some(signal) = signal else { return };

    let (tx, rx) = mpsc::unbounded_channel::<VoiceCommand>();
    {
        let mut s = state.write().await;

        if let Some(old) = s.voice_cmd_tx.take() {
            crate::info!("maybe_start_voice: disconnecting previous CALL voice manager");
            let _ = old.send(VoiceCommand::Disconnect);
        }
        crate::info!("maybe_start_voice: spawning CALL voice manager");
        s.voice_cmd_tx = Some(tx.clone());
        s.voice_status = "voice: connecting…".to_string();

        s.joining_voice_token = None;
        s.joining_voice_endpoint = None;
        s.joining_voice_session = None;

        if s.self_mute || s.self_deaf {
            let _ = tx.send(VoiceCommand::SetMute(s.self_mute));
            let _ = tx.send(VoiceCommand::SetDeaf(s.self_deaf));
        }
    }

    let st = state.clone();
    tokio::spawn(async move {
        if let Err(e) = VoiceManager::run(signal, rx).await {
            error!("voice manager: {e}");
            st.write().await.voice_status = format!("voice error: {e}");
        } else {
            st.write().await.voice_status = "voice: disconnected".to_string();
        }
    });
}

async fn maybe_start_stream(state: &SharedState, session_id: &str, stream_key: &str) {
    let signal = {
        let s = state.read().await;
        let Some(p) = s.pending_streams.get(stream_key) else {
            return;
        };
        if s.stream_conns.contains_key(&p.uid) {
            return;
        }
        match (&p.token, &p.endpoint, &p.server, s.my_user_id) {
            (Some(token), Some(endpoint), Some(server), Some(uid)) if p.uid != 0 => Some((
                p.uid,
                VoiceSignal {
                    guild_id: server.clone(),
                    user_id: uid.to_string(),
                    channel_id: s
                        .voice_channel
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "0".to_string()),
                    session_id: session_id.to_string(),
                    token: token.clone(),
                    endpoint: endpoint.clone(),
                    input_device: None,
                    output_device: None,
                    dsp_rx: false,
                    dsp_tx: false,
                    watch_uid: Some(p.uid),
                    server_id_override: Some(server.clone()),
                    broadcast: false,
                },
            )),
            _ => None,
        }
    };
    let Some((buid, signal)) = signal else { return };

    let (tx, rx) = mpsc::unbounded_channel::<VoiceCommand>();
    {
        let mut s = state.write().await;
        if let Some(old) = s.stream_conns.insert(buid, tx) {
            crate::info!(
                "maybe_start_stream: REPLACING existing watch conn for uid={buid} (disconnecting old)"
            );
            let _ = old.send(VoiceCommand::Disconnect);
        } else {
            crate::info!("maybe_start_stream: spawning watch conn for uid={buid}");
        }
        s.voice_status = format!("stream {buid}: connecting…");
    }

    let st = state.clone();
    tokio::spawn(async move {
        if let Err(e) = VoiceManager::run(signal, rx).await {
            error!("stream manager: {e}");
            st.write().await.voice_status = format!("stream error: {e}");
        } else {
            st.write().await.voice_status = "stream: ended".to_string();
        }
    });
}

async fn maybe_start_golive(state: &SharedState, session_id: &str, stream_key: &str) {
    let signal = {
        let s = state.read().await;
        if s.golive_cmd_tx.is_some() {
            return;
        }
        let Some(p) = s.pending_streams.get(stream_key) else {
            return;
        };
        match (&p.token, &p.endpoint, &p.server, s.my_user_id) {
            (Some(token), Some(endpoint), Some(server), Some(uid)) => Some(VoiceSignal {
                guild_id: server.clone(),
                user_id: uid.to_string(),
                channel_id: s
                    .voice_channel
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "0".to_string()),
                session_id: session_id.to_string(),
                token: token.clone(),
                endpoint: endpoint.clone(),
                input_device: None,
                output_device: None,
                dsp_rx: false,
                dsp_tx: false,
                watch_uid: None,
                server_id_override: Some(server.clone()),
                broadcast: true,
            }),
            _ => None,
        }
    };
    let Some(signal) = signal else { return };

    let (tx, rx) = mpsc::unbounded_channel::<VoiceCommand>();
    {
        let mut s = state.write().await;
        if let Some(old) = s.golive_cmd_tx.replace(tx) {
            let _ = old.send(VoiceCommand::Disconnect);
        }
        s.voice_status = "Go Live: connecting…".to_string();
    }
    crate::info!("maybe_start_golive: spawning broadcaster RTC connection");
    let st = state.clone();
    tokio::spawn(async move {
        if let Err(e) = VoiceManager::run(signal, rx).await {
            error!("golive manager: {e}");
            st.write().await.voice_status = format!("Go Live error: {e}");
        } else {
            st.write().await.voice_status = "Go Live: ended".to_string();
        }
    });
}

async fn apply_ready(d: ReadyData, state: &SharedState) {
    let my_user = d.user.as_ref().map(|u| (u.id, u.username.clone()));

    let mut guilds = Vec::with_capacity(d.guilds.len() + 1);
    let mut channels_by_guild = std::collections::HashMap::new();

    if let Some(dms) = &d.private_channels {
        let user_table: std::collections::HashMap<&str, (&str, Option<&str>)> = d
            .users
            .iter()
            .map(|u| (u.id.as_str(), (u.username.as_str(), u.avatar.as_deref())))
            .collect();
        guilds.push(GuildSummary {
            id: DM_GUILD_ID,
            name: "@ Direct Messages".to_string(),
            icon_url: None,
        });
        let mut dm_channels: Vec<ChannelSummary> = dms
            .iter()
            .map(|dm| ChannelSummary {
                id: dm.id,
                kind: dm.type_field,
                position: 0,
                parent_id: None,
                name: dm_label(dm, &user_table),

                recipient_id: if dm.type_field == 1 {
                    dm.recipient_ids
                        .first()
                        .and_then(|s| s.parse::<u64>().ok())
                        .map(SnowflakeID::from_u64)
                        .or_else(|| {
                            dm.recipients
                                .first()
                                .and_then(|r| r.id.parse::<u64>().ok())
                                .map(SnowflakeID::from_u64)
                        })
                } else {
                    None
                },
                last_activity: dm.last_message_id.map(u64::from).unwrap_or(0),
                avatar_url: dm_avatar_url(dm, &user_table),
            })
            .collect();
        dm_channels.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
        channels_by_guild.insert(DM_GUILD_ID, dm_channels);
    }

    let mut voice_members: std::collections::HashMap<
        SnowflakeID,
        Vec<crate::tui::state::VoiceMember>,
    > = std::collections::HashMap::new();

    for g in &d.guilds {
        let name = g.display_name().to_string();

        let icon_hash = g
            .properties
            .as_ref()
            .and_then(|p| p.icon.clone())
            .or_else(|| g.icon.clone());
        let icon_url = icon_hash.map(|h| icon_url_for(g.id, &h));
        guilds.push(GuildSummary {
            id: g.id,
            name,
            icon_url,
        });
        channels_by_guild.insert(g.id, guild_channel_summaries(g));

        for vs in &g.voice_states {
            let Some(uid) = vs
                .get("user_id")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u64>().ok())
                .map(SnowflakeID::from_u64)
            else {
                continue;
            };
            let Some(ch) = vs
                .get("channel_id")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u64>().ok())
                .map(SnowflakeID::from_u64)
            else {
                continue;
            };
            let name = vs
                .get("member")
                .and_then(|m| m.get("user"))
                .and_then(|u| u.get("username"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .unwrap_or_else(|| uid.to_string());
            let b = |k: &str| vs.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
            voice_members
                .entry(ch)
                .or_default()
                .push(crate::tui::state::VoiceMember {
                    id: uid,
                    name,
                    mute: b("mute") || b("self_mute"),
                    deaf: b("deaf") || b("self_deaf"),
                    video: b("self_video"),
                    stream: b("self_stream"),
                });
        }
    }

    let mut user_dir: std::collections::HashMap<SnowflakeID, String> =
        std::collections::HashMap::new();
    for u in &d.users {
        if let Ok(id) = u.id.parse::<u64>() {
            let name = u
                .global_name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| u.username.trim());
            if !name.is_empty() {
                user_dir.insert(SnowflakeID::from_u64(id), name.to_string());
            }
        }
    }

    let mut s = state.write().await;
    s.guilds = guilds;
    s.channels_by_guild = channels_by_guild;
    s.voice_members = voice_members;
    for (id, name) in user_dir {
        s.user_dir.insert(id, name);
    }
    if let Some((id, name)) = my_user {
        s.my_user_id = Some(id);
        s.user_dir.entry(id).or_insert_with(|| name.clone());
        s.my_username = name;
    }
    if s.current_guild.is_none() {
        s.current_guild = s.guilds.first().map(|g| g.id);
    }
    let n = s.guilds.len();
    s.conn_status = "online".to_string();
    s.set_status(format!("READY — {n} servers"));
}

fn guild_channel_summaries(g: &Guild) -> Vec<ChannelSummary> {
    let mut chans: Vec<ChannelSummary> = g
        .channels
        .iter()
        .map(|c| ChannelSummary {
            id: c.id,
            kind: c.type_field,
            position: c.position,
            parent_id: c.parent_id,
            name: c.name.clone(),
            recipient_id: None,
            last_activity: c.last_message_id.map(u64::from).unwrap_or(0),
            avatar_url: None,
        })
        .collect();

    let mut threads_by_parent: std::collections::HashMap<u64, Vec<ChannelSummary>> =
        std::collections::HashMap::new();
    for t in &g.threads {
        if let Some(p) = t.parent_id {
            threads_by_parent
                .entry(u64::from(p))
                .or_default()
                .push(ChannelSummary {
                    id: t.id,
                    kind: t.type_field,
                    position: 0,
                    parent_id: t.parent_id,
                    name: format!("↳ {}", t.name),
                    recipient_id: None,
                    last_activity: t.last_message_id.map(u64::from).unwrap_or(0),
                    avatar_url: None,
                });
        }
    }

    let group = |k: i64| if matches!(k, 2 | 13) { 1 } else { 0 };
    chans.sort_by_key(|c| (group(c.kind), c.position, u64::from(c.id)));

    let mut categories: Vec<&ChannelSummary> = chans.iter().filter(|c| c.kind == 4).collect();
    categories.sort_by_key(|c| (c.position, u64::from(c.id)));

    let mut out: Vec<ChannelSummary> = Vec::with_capacity(chans.len());
    let mut push_with_threads = |out: &mut Vec<ChannelSummary>, c: &ChannelSummary| {
        out.push(c.clone());
        if let Some(ts) = threads_by_parent.get(&u64::from(c.id)) {
            for t in ts {
                out.push(t.clone());
            }
        }
    };

    for c in chans
        .iter()
        .filter(|c| c.kind != 4 && c.parent_id.is_none())
    {
        push_with_threads(&mut out, c);
    }
    for cat in categories {
        out.push(cat.clone());
        for c in chans
            .iter()
            .filter(|c| c.kind != 4 && c.parent_id == Some(cat.id))
        {
            push_with_threads(&mut out, c);
        }
    }
    out
}

type UserTable<'a> = std::collections::HashMap<&'a str, (&'a str, Option<&'a str>)>;

fn dm_label(dm: &crate::receive_struct::ready::PrivateChannels, user_table: &UserTable) -> String {
    if let Some(name) = &dm.name {
        if !name.is_empty() {
            return name.clone();
        }
    }
    if !dm.recipients.is_empty() {
        return dm
            .recipients
            .iter()
            .map(|r| r.username.as_str())
            .collect::<Vec<_>>()
            .join(", ");
    }
    dm.recipient_ids
        .iter()
        .map(|rid| user_table.get(rid.as_str()).map(|t| t.0).unwrap_or("?"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn dm_avatar_url(
    dm: &crate::receive_struct::ready::PrivateChannels,
    user_table: &UserTable,
) -> Option<String> {
    if dm.type_field != 1 {
        return None;
    }
    if let Some(r) = dm.recipients.first() {
        let avatar = r.avatar.as_deref()?;
        return Some(avatar_url_for(&r.id, avatar));
    }
    let rid = dm.recipient_ids.first()?.as_str();
    let avatar = user_table.get(rid).and_then(|t| t.1)?;
    Some(avatar_url_for(rid, avatar))
}

fn icon_url_for(guild_id: SnowflakeID, hash: &str) -> String {
    let ext = if hash.starts_with("a_") { "gif" } else { "png" };
    format!("https://cdn.discordapp.com/icons/{guild_id}/{hash}.{ext}?size=4096")
}

fn avatar_url_for(user_id: &str, hash: &str) -> String {
    let ext = if hash.starts_with("a_") { "gif" } else { "png" };
    format!("https://cdn.discordapp.com/avatars/{user_id}/{hash}.{ext}?size=64")
}

fn cache_dir() -> std::path::PathBuf {
    let p = std::env::current_dir()
        .unwrap_or_else(|_| ".".into())
        .join("img_cache");
    let _ = std::fs::create_dir_all(&p);
    p
}

fn cache_path(id: SnowflakeID) -> std::path::PathBuf {
    cache_dir().join(format!("{id}.bin"))
}

fn load_from_disk(id: SnowflakeID) -> Option<image::DynamicImage> {
    let bytes = std::fs::read(cache_path(id)).ok()?;
    image::load_from_memory(&bytes).ok()
}

fn save_to_disk(id: SnowflakeID, bytes: &[u8]) {
    let _ = std::fs::write(cache_path(id), bytes);
}

pub async fn fetch_thumbnails(state: SharedState) {
    use crate::utils::http_client;

    let jobs: Vec<(SnowflakeID, String)> = {
        let s = state.read().await;
        let mut v: Vec<(SnowflakeID, String)> = s
            .guilds
            .iter()
            .filter_map(|g| g.icon_url.as_ref().map(|u| (g.id, u.clone())))
            .collect();
        if let Some(dms) = s.channels_by_guild.get(&DM_GUILD_ID) {
            v.extend(
                dms.iter()
                    .filter_map(|c| c.avatar_url.as_ref().map(|u| (c.id, u.clone())))
                    .take(64),
            );
        }
        v
    };

    let mut from_disk = 0usize;
    let mut from_net = 0usize;
    let mut failed = 0usize;
    let total = jobs.len();

    for (id, url) in jobs {
        if state.read().await.image_cache.contains_key(&id) {
            continue;
        }
        if let Some(img) = load_from_disk(id) {
            state.write().await.image_cache.insert(id, img);
            from_disk += 1;
            continue;
        }
        let resp = match http_client().get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                failed += 1;
                if failed <= 3 {
                    crate::error!("thumb: GET {url} failed: {e}");
                }
                continue;
            }
        };
        if !resp.status().is_success() {
            failed += 1;
            if failed <= 3 {
                crate::error!("thumb: {url} HTTP {}", resp.status());
            }
            continue;
        }
        let Ok(bytes) = resp.bytes().await else {
            failed += 1;
            continue;
        };
        let img = match image::load_from_memory(&bytes) {
            Ok(i) => i,
            Err(e) => {
                failed += 1;
                if failed <= 3 {
                    crate::error!("thumb: decode {url} failed ({} bytes): {e}", bytes.len());
                }
                continue;
            }
        };
        save_to_disk(id, &bytes);
        let (w, h) = (img.width(), img.height());
        state.write().await.image_cache.insert(id, img);
        from_net += 1;
        if from_net <= 3 {
            crate::info!("thumb: decoded {url} → {w}x{h}");
        }
    }

    let msg = format!(
        "thumbnails: {from_disk} cached, {from_net} downloaded, {failed} failed (of {total})"
    );
    crate::info!("{msg}");
    state.write().await.set_status(msg);
}

pub fn display_message_from(
    d: &MessageCreateData,
    my_id: Option<SnowflakeID>,
) -> (DisplayMessage, Vec<(SnowflakeID, String)>) {
    let ts = chrono::DateTime::parse_from_rfc3339(&d.timestamp)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());
    let attachment_images = extract_image_attachments(d);
    let mut content = render_message_content(d);
    for e in &d.embeds {
        let t = e.title.as_deref().unwrap_or("");
        let de = e.description.as_deref().unwrap_or("");
        if !t.is_empty() || !de.is_empty() {
            let snip: String = de.replace('\n', " ").chars().take(120).collect();
            content.push_str(&format!("\n  ▎embed: {t} {snip}"));
        }
    }
    let msg = DisplayMessage {
        id: d.id,
        channel_id: d.channel_id,
        author: d
            .author
            .global_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(d.author.username.trim())
            .to_string(),
        author_id: d.author.id,
        content,
        timestamp: ts,
        edited: d.edited_timestamp.is_some(),
        is_self: my_id.map(|mid| d.author.id == mid).unwrap_or(false),
        attachment_images: attachment_images.clone(),
        reply_to: d.referenced_message.as_ref().map(|r| {
            let who = r
                .author
                .global_name
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(r.author.username.as_str())
                .to_string();
            let snip: String = r.content.replace('\n', " ").chars().take(80).collect();
            (who, snip)
        }),
        reactions: d
            .reactions
            .as_ref()
            .map(|rs| {
                rs.iter()
                    .map(|r| (r.name.clone(), r.count as u32))
                    .collect()
            })
            .unwrap_or_default(),
    };
    (msg, attachment_images)
}

pub fn display_message_from_value(
    v: &serde_json::Value,
    my_id: Option<SnowflakeID>,
) -> (DisplayMessage, Vec<(SnowflakeID, String)>) {
    if let Ok(d) = serde_json::from_value::<MessageCreateData>(v.clone()) {
        return display_message_from(&d, my_id);
    }

    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let author = v.get("author");
    let author_name = author
        .and_then(|a| a.get("username"))
        .and_then(|x| x.as_str())
        .filter(|x| !x.is_empty())
        .or_else(|| {
            author
                .and_then(|a| a.get("global_name"))
                .and_then(|x| x.as_str())
        })
        .unwrap_or("unknown")
        .to_string();
    let author_id = author
        .and_then(|a| a.get("id"))
        .and_then(|x| x.as_str())
        .and_then(|x| x.parse::<u64>().ok())
        .map(SnowflakeID::from_u64)
        .unwrap_or_default();
    let id = v
        .get("id")
        .and_then(|x| x.as_str())
        .and_then(|x| x.parse::<u64>().ok())
        .map(SnowflakeID::from_u64)
        .unwrap_or_default();
    let channel_id = v
        .get("channel_id")
        .and_then(|x| x.as_str())
        .and_then(|x| x.parse::<u64>().ok())
        .map(SnowflakeID::from_u64)
        .unwrap_or_default();
    let ts = chrono::DateTime::parse_from_rfc3339(&s("timestamp"))
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());

    let mut imgs: Vec<(SnowflakeID, String)> = Vec::new();
    if let Some(arr) = v.get("attachments").and_then(|a| a.as_array()) {
        for a in arr {
            let url = a.get("url").and_then(|u| u.as_str());
            let ct = a.get("content_type").and_then(|u| u.as_str()).unwrap_or("");
            let fname = a.get("filename").and_then(|u| u.as_str()).unwrap_or("");
            let is_img = ct.starts_with("image/")
                || [".png", ".jpg", ".jpeg", ".gif", ".webp"]
                    .iter()
                    .any(|e| fname.to_ascii_lowercase().ends_with(e));
            if let (Some(url), true) = (url, is_img) {
                if let Some(aid) = a
                    .get("id")
                    .and_then(|x| x.as_str())
                    .and_then(|x| x.parse::<u64>().ok())
                {
                    imgs.push((SnowflakeID::from_u64(aid), url.to_string()));
                }
            }
        }
    }

    let content = {
        let base = s("content");
        if base.is_empty() && !imgs.is_empty() {
            format!("[{} image(s)]", imgs.len())
        } else if base.is_empty() {
            "[no content]".to_string()
        } else {
            base
        }
    };

    let msg = DisplayMessage {
        id,
        channel_id,
        author: author_name,
        author_id,
        content,
        timestamp: ts,
        edited: v
            .get("edited_timestamp")
            .map(|x| !x.is_null())
            .unwrap_or(false),
        is_self: my_id.map(|mid| author_id == mid).unwrap_or(false),
        attachment_images: imgs.clone(),
        reply_to: v.get("referenced_message").and_then(|r| {
            let a = r.get("author")?;
            let who = a
                .get("global_name")
                .and_then(|x| x.as_str())
                .filter(|x| !x.is_empty())
                .or_else(|| a.get("username").and_then(|x| x.as_str()))
                .unwrap_or("unknown")
                .to_string();
            let snip: String = r
                .get("content")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .replace('\n', " ")
                .chars()
                .take(80)
                .collect();
            Some((who, snip))
        }),
        reactions: v
            .get("reactions")
            .and_then(|x| x.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        let name = r
                            .get("emoji")
                            .and_then(|e| e.get("name"))
                            .or_else(|| r.get("name"))
                            .and_then(|x| x.as_str())?
                            .to_string();
                        let count = r.get("count").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
                        Some((name, count))
                    })
                    .collect()
            })
            .unwrap_or_default(),
    };
    (msg, imgs)
}

pub fn spawn_image_fetches(state: &SharedState, images: Vec<(SnowflakeID, String)>) {
    for (id, url) in images {
        let s = state.clone();
        tokio::spawn(async move { fetch_one_image(&s, id, &url).await });
    }
}

async fn apply_message(d: MessageCreateData, state: &SharedState) {
    let channel_id = d.channel_id;
    let activity = u64::from(d.id);
    let my_id = state.read().await.my_user_id;
    let (msg, attachment_images) = display_message_from(&d, my_id);

    spawn_image_fetches(state, attachment_images);

    let mut s = state.write().await;
    s.note_user(
        d.author.id,
        d.author.global_name.as_deref(),
        &d.author.username,
    );

    if let Some(v) = s.typing.get_mut(&channel_id) {
        v.retain(|(u, _)| *u != d.author.id);
    }
    s.push_message(channel_id, msg);

    let is_current = s.current_channel == Some(channel_id);
    let is_self = my_id.map(|m| m == d.author.id).unwrap_or(false);
    if !is_current && !is_self {
        s.unread.insert(channel_id);
        *s.unread_count.entry(channel_id).or_insert(0) += 1;
        let owner = s
            .channels_by_guild
            .iter()
            .find(|(_, chans)| chans.iter().any(|c| c.id == channel_id))
            .map(|(gid, _)| *gid);
        if let Some(gid) = owner {
            s.unread.insert(gid);
        }
        if owner == Some(DM_GUILD_ID) {
            let who = s.display_name(d.author.id);
            let body: String = d.content.chars().take(160).collect();
            s.push_toast(who, body);
        } else if s.prefs.notify_mentions {
            let mentions_me = my_id
                .map(|id| d.content.contains(&format!("<@{}", u64::from(id))))
                .unwrap_or(false)
                || d.content.contains("@everyone")
                || d.content.contains("@here");
            if mentions_me {
                let who = s.display_name(d.author.id);
                let body: String = d.content.chars().take(160).collect();
                s.push_toast(format!("@ {who}"), body);
            }
        }
    }

    if let Some(dms) = s.channels_by_guild.get_mut(&DM_GUILD_ID) {
        if let Some(pos) = dms.iter().position(|c| c.id == channel_id) {
            dms[pos].last_activity = activity;
            let item = dms.remove(pos);
            dms.insert(0, item);

            if s.current_guild == Some(DM_GUILD_ID) {
                if let Some(selected) = s.current_channel {
                    if let Some(new_pos) = s.channels_by_guild[&DM_GUILD_ID]
                        .iter()
                        .position(|c| c.id == selected)
                    {
                        s.channel_cursor = new_pos;
                    }
                }
            }
        }
    }
}

async fn apply_message_delete(d: MessageDeleteData, state: &SharedState) {
    let mut s = state.write().await;
    if let Some(buf) = s.messages.get_mut(&d.channel_id) {
        buf.retain(|m| m.id != d.id);
    }
}

fn render_message_content(d: &MessageCreateData) -> String {
    use std::fmt::Write as _;

    let system = match d.type_field {
        0 | 19 | 20 | 21 | 23 | 24 => None,
        1 => Some("[added to the group]".to_string()),
        2 => Some("[removed from the group]".to_string()),
        3 => Some("[started a call]".to_string()),
        4 => Some(format!("[renamed the channel to «{}»]", d.content)),
        5 => Some("[changed the channel icon]".to_string()),
        6 => Some("[pinned a message]".to_string()),
        7 => Some("[joined the server]".to_string()),
        8 => Some("[boosted the server]".to_string()),
        9 => Some("[boosted the server — tier 1]".to_string()),
        10 => Some("[boosted the server — tier 2]".to_string()),
        11 => Some("[boosted the server — tier 3]".to_string()),
        12 => Some("[followed an announcement channel]".to_string()),
        14 => Some("[guild discovery disqualified]".to_string()),
        15 => Some("[guild discovery requalified]".to_string()),
        16 => Some("[guild discovery grace period — initial warning]".to_string()),
        17 => Some("[guild discovery grace period — final warning]".to_string()),
        18 => Some(format!("[thread created] {}", d.content)),
        22 => Some("[guild invite reminder]".to_string()),
        25 => Some("[role subscription purchased]".to_string()),
        26 => Some("[premium upsell]".to_string()),
        27 => Some("[stage started]".to_string()),
        28 => Some("[stage ended]".to_string()),
        29 => Some("[stage speaker changed]".to_string()),
        31 => Some(format!("[stage topic] {}", d.content)),
        32 => Some("[guild app premium subscription]".to_string()),
        36 => Some("[server incident alert mode enabled]".to_string()),
        37 => Some("[server incident alert mode disabled]".to_string()),
        38 => Some("[raid reported]".to_string()),
        39 => Some("[raid report flagged false alarm]".to_string()),
        44 => Some("[purchase]".to_string()),
        46 => Some("[poll result]".to_string()),
        other => Some(format!("[unknown message type {other}]")),
    };

    if let Some(s) = system {
        return s;
    }

    let mut out = String::with_capacity(d.content.len() + 32);

    if let Some(inter) = &d.interaction {
        let _ = write!(out, "[/{}] ", inter.name);
    }

    if !d.content.is_empty() {
        out.push_str(&d.content);
    }

    if let Some(sticks) = &d.sticker_items {
        for s in sticks {
            if !out.is_empty() {
                out.push(' ');
            }
            let _ = write!(out, "[sticker:{}]", s.name);
        }
    }

    for a in &d.attachments {
        if !out.is_empty() {
            out.push(' ');
        }
        let _ = write!(out, "[file:{}]", a.filename);
    }

    for e in &d.embeds {
        let label = e
            .title
            .as_deref()
            .or(e.description.as_deref())
            .map(|s| s.lines().next().unwrap_or(""))
            .unwrap_or("");
        if !out.is_empty() {
            out.push(' ');
        }
        if label.is_empty() {
            let _ = write!(out, "[embed]");
        } else {
            let trimmed: String = label.chars().take(80).collect();
            let _ = write!(out, "[embed: {trimmed}]");
        }
    }

    if d.poll.is_some() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str("[poll]");
    }

    if out.is_empty() {
        out.push_str("[empty]");
    }

    out
}

impl AppState {
    #[allow(dead_code)]
    fn _dm_guild_id() -> SnowflakeID {
        DM_GUILD_ID
    }
}

fn extract_image_attachments(d: &MessageCreateData) -> Vec<(SnowflakeID, String)> {
    use std::hash::{Hash, Hasher};
    let mut out = Vec::new();

    for a in &d.attachments {
        let is_image = a
            .content_type
            .as_deref()
            .map(|ct| ct.starts_with("image/"))
            .unwrap_or_else(|| {
                let fname = a.filename.to_ascii_lowercase();
                fname.ends_with(".png")
                    || fname.ends_with(".jpg")
                    || fname.ends_with(".jpeg")
                    || fname.ends_with(".gif")
                    || fname.ends_with(".webp")
            });
        if !is_image {
            continue;
        }
        let Some(url) = a.url.clone() else { continue };
        let Ok(id) = a.id.parse::<u64>() else {
            continue;
        };
        out.push((SnowflakeID::from_u64(id), url));
    }

    for embed in &d.embeds {
        let url_opt = embed
            .image
            .as_ref()
            .and_then(|i| i.url.clone())
            .or_else(|| embed.thumbnail.as_ref().and_then(|t| t.url.clone()));
        let Some(url) = url_opt else { continue };

        let mut h = std::collections::hash_map::DefaultHasher::new();
        url.hash(&mut h);
        out.push((SnowflakeID::from_u64(h.finish()), url));
    }

    out
}

async fn fetch_one_image(state: &SharedState, id: SnowflakeID, url: &str) {
    if state.read().await.image_cache.contains_key(&id) {
        return;
    }
    if let Some(img) = load_from_disk(id) {
        state.write().await.image_cache.insert(id, img);
        return;
    }
    let Ok(resp) = crate::utils::http_client().get(url).send().await else {
        return;
    };
    if !resp.status().is_success() {
        return;
    }
    let Ok(bytes) = resp.bytes().await else {
        return;
    };
    let Ok(img) = image::load_from_memory(&bytes) else {
        return;
    };
    save_to_disk(id, &bytes);
    state.write().await.image_cache.insert(id, img);
}
