#![allow(dead_code)]
#![allow(unused)]
#![allow(deprecated)]
#![allow(clippy::all)]

use std::error::Error;
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use std::{collections::HashMap, env, fs, fs::File};

use chrono::{DateTime, Local};
use comparable::{Changed, Comparable, VecChange};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::select;

use receive_struct::*;
use send_struct::*;

use crate::config::config::{Config, ConfigChange, EventChange, WriteFileChange};
use crate::config::make_config_file;
use crate::discord_connection::{GatewaySink, GatewayStream};
use crate::heart_beat::heart_beat;
use crate::message_buffer::MessageBuffer;
use crate::receive_struct::gateway::{GatewayEnvelope, opcode};
use crate::receive_struct::hello::HelloData;
use crate::receive_struct::receive_event::DiscordEventMessage;
use crate::utils::{SnowflakeID, timestamp_to_date};

mod config;
mod discord_connection;
mod heart_beat;
mod mapping_struct;
mod message_buffer;
mod receive_struct;
mod rest;
mod send_struct;
mod tui;
mod utils;
mod voice;

const GATEWAY_URL: &str = "wss://gateway.discord.gg/?encoding=json&v=10";

macro_rules! ignore_err {
    ($expr:expr, $ctx:expr) => {
        if let Err(e) = $expr {
            error!("{}: {}", $ctx, e);
        }
    };
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let base_path = env::current_dir()?;
    let config = make_config_file(&base_path.join("config.json")).await?;

    let mode = env::args().nth(1).unwrap_or_else(|| "--dump".to_string());
    match mode.as_str() {
        "--tui" | "tui" => return tui::run(config).await,
        "--dump" | "dump" => {}
        other => {
            eprintln!("unknown mode {other}; using --dump");
        }
    }

    dump_mode(config, base_path).await
}

async fn dump_mode(
    mut config: Config,
    base_path: std::path::PathBuf,
) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(base_path.join("dm/"))?;

    let (mut socket_sender, mut socket_reader) =
        discord_connection::discord_wss_connection(GATEWAY_URL).await?;

    let mut hb_timer = tokio::time::interval(Duration::from_millis(41_250));
    hb_timer.tick().await;

    let mut reload_json = tokio::time::interval(Duration::from_secs(30));
    reload_json.tick().await;

    let mut last_seq: Option<u64> = None;
    let mut message_buffer = MessageBuffer::new(config.message_buffer_size);
    let mut g_session_id = String::new();
    let mut last_channel_id = None;

    info!("Send Payload");
    payload(&mut socket_sender, &config.token).await?;

    ignore_err!(
        _request_guild_members(&mut socket_sender, "1476763563409539275", "", 0).await,
        "request_guild_members"
    );

    info!("crash dioscord cve");
    crash_discord(&mut socket_sender).await;

    let mut exit_on_error = 0;

    info!("Mapping server / channel");
    info!("Message buffer: {}", config.message_buffer_size);

    let mut dump_file = DumpFile {
        ready_json: File::create(base_path.join("ready.json"))?,
        ready_supplemental_json: File::create(base_path.join("ready_supplemental.json"))?,
        server_channel_dump: File::create(base_path.join("server_channel.json"))?,
        dm_dump_file: File::create(base_path.join("dm_dump.json"))?,
        tracking_dump: fs::OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(base_path.join("tracking.txt"))?,
        todo_dump: File::create(base_path.join("todo_dump.txt"))?,
    };

    let mut data_id = DataId {
        guild_id: HashMap::new(),
        channel_id: HashMap::new(),
        dm_dump_hashmap: HashMap::new(),
        muted_server: HashMap::new(),
        username: config.dm_channel_id_tracking.clone(),
        session_id: String::new(),
        resume_gateway: String::new(),
    };

    writeln!(
        &mut dump_file.tracking_dump,
        "\n{a} {b} {a}",
        a = "-".repeat(10),
        b = Local::now(),
    )?;

    loop {
        select! {
            msg_base = socket_reader.next() => {
                match msg_base {
                    Some(Ok(msg)) => {
                        exit_on_error = 0;
                        let msg_text = match msg.to_text() {
                            Ok(t) => t,
                            Err(e) => {
                                error!("non-text gateway frame: {e}");
                                continue;
                            }
                        };

                        let envelope: GatewayEnvelope = match serde_json::from_str(msg_text) {
                            Ok(env) => env,
                            Err(e) => {
                                if config.debug { serde_parsing_error(&e, msg_text); }
                                continue;
                            }
                        };

                        if let Some(s) = envelope.s { last_seq = Some(s); }

                        if let Err(e) = handle_envelope(
                            envelope,
                            msg_text,
                            &mut socket_sender,
                            &mut socket_reader,
                            &mut hb_timer,
                            &mut dump_file,
                            &mut data_id,
                            &mut g_session_id,
                            &config,
                            &mut last_channel_id,
                            &mut last_seq,
                            &mut message_buffer,
                            &base_path,
                        ).await {
                            error!("event handler error: {e}");
                        }
                    }
                    Some(Err(e)) => {
                        error!("websocket read error: {e}");
                        if let Err(re) = try_resume_or_reconnect(
                            &mut socket_sender,
                            &mut socket_reader,
                            &config.token,
                            &data_id,
                            last_seq,
                        ).await {
                            error!("resume/reconnect failed: {re}");
                            data_id.clear();
                            last_seq = None;
                        }
                        writeln!(
                            &mut dump_file.tracking_dump,
                            "\n{} {} {}",
                            "-".repeat(10),
                            Local::now(),
                            "-".repeat(10)
                        ).ok();
                    }
                    None => {
                        exit_on_error += 1;
                        if exit_on_error >= 5 {
                            error!("5 consecutive empty reads, exiting");
                            return Ok(());
                        }
                        ignore_err!(socket_sender.close().await, "closing socket");
                        let (sender, reader) = discord_connection::discord_wss_connection(GATEWAY_URL).await?;
                        socket_sender = sender;
                        socket_reader = reader;
                        payload(&mut socket_sender, &config.token).await?;
                        data_id.clear();
                        last_seq = None;
                        writeln!(
                            &mut dump_file.tracking_dump,
                            "\n{} {} {}",
                            "-".repeat(10),
                            Local::now(),
                            "-".repeat(10)
                        ).ok();
                    }
                }
            }

            _ = hb_timer.tick() => {
                ignore_err!(heart_beat(&mut socket_sender, last_seq).await, "heartbeat");
            }

            _ = reload_json.tick() => {
                let tmp_config = config.clone();
                match make_config_file(&base_path.join("config.json")).await {
                    Ok(new_cfg) => {
                        config = new_cfg;
                        config_change(tmp_config.comparison(&config), &mut dump_file.tracking_dump);
                    }
                    Err(e) => error!("config reload: {e}"),
                }
            }
        }
    }
}

async fn handle_envelope(
    envelope: GatewayEnvelope,
    raw_text: &str,
    socket_sender: &mut GatewaySink,
    socket_reader: &mut GatewayStream,
    hb_timer: &mut tokio::time::Interval,
    dump_file: &mut DumpFile,
    data_id: &mut DataId,
    g_session_id: &mut String,
    config: &Config,
    last_channel_id: &mut Option<SnowflakeID>,
    last_seq: &mut Option<u64>,
    message_buffer: &mut MessageBuffer,
    base_path: &Path,
) -> Result<(), Box<dyn Error>> {
    match envelope.op {
        opcode::DISPATCH => {
            let event: DiscordEventMessage = match serde_json::from_str(raw_text) {
                Ok(ev) => ev,
                Err(e) => {
                    if config.write_file.todo_dump {
                        writeln!(&mut dump_file.todo_dump, "{},\n", raw_text).ok();
                    }
                    if config.debug {
                        serde_parsing_error(&e, raw_text);
                    }
                    return Ok(());
                }
            };
            dispatch_event(
                event,
                raw_text,
                dump_file,
                data_id,
                g_session_id,
                config,
                last_channel_id,
                message_buffer,
                base_path,
            )
            .await?;
        }
        opcode::HELLO => match serde_json::from_value::<HelloData>(envelope.d) {
            Ok(hello) => {
                info!("HELLO heartbeat_interval={}ms", hello.heartbeat_interval);
                *hb_timer = tokio::time::interval(Duration::from_millis(hello.heartbeat_interval));
                hb_timer.tick().await;
                heart_beat(socket_sender, *last_seq).await?;
            }
            Err(e) => error!("HELLO parse: {e}"),
        },
        opcode::HEARTBEAT => {
            heart_beat(socket_sender, *last_seq).await?;
        }
        opcode::HEARTBEAT_ACK => {}
        opcode::RECONNECT => {
            warn!("RECONNECT requested by gateway");
            try_resume_or_reconnect(
                socket_sender,
                socket_reader,
                &config.token,
                data_id,
                *last_seq,
            )
            .await?;
        }
        opcode::INVALID_SESSION => {
            let resumable = envelope.d.as_bool().unwrap_or(false);
            warn!("INVALID_SESSION (resumable={})", resumable);
            if resumable && !data_id.session_id.is_empty() {
                discord_connection::discord_wss_resume(
                    socket_sender,
                    &config.token,
                    &data_id.session_id,
                    last_seq.unwrap_or(0),
                )
                .await?;
            } else {
                tokio::time::sleep(Duration::from_secs(2)).await;
                payload(socket_sender, &config.token).await?;
                data_id.clear();
                *last_seq = None;
            }
        }
        op => {
            if config.debug {
                warn!("unhandled opcode {op}");
            }
        }
    }
    Ok(())
}

async fn try_resume_or_reconnect(
    socket_sender: &mut GatewaySink,
    socket_reader: &mut GatewayStream,
    token: &str,
    data_id: &DataId,
    last_seq: Option<u64>,
) -> Result<(), Box<dyn Error>> {
    let _ = socket_sender.close().await;
    let _ = socket_reader.next().await;

    let url = if !data_id.resume_gateway.is_empty() {
        format!(
            "{}/?encoding=json&v=10",
            data_id.resume_gateway.trim_end_matches('/')
        )
    } else {
        GATEWAY_URL.to_string()
    };
    let (sender, reader) = discord_connection::discord_wss_connection(&url).await?;
    *socket_sender = sender;
    *socket_reader = reader;

    if !data_id.session_id.is_empty() && last_seq.is_some() {
        discord_connection::discord_wss_resume(
            socket_sender,
            token,
            &data_id.session_id,
            last_seq.unwrap_or(0),
        )
        .await?;
    } else {
        payload(socket_sender, token).await?;
    }
    Ok(())
}

async fn dispatch_event(
    event: DiscordEventMessage,
    raw_text: &str,
    dump_file: &mut DumpFile,
    data_id: &mut DataId,
    g_session_id: &mut String,
    config: &Config,
    last_channel_id: &mut Option<SnowflakeID>,
    message_buffer: &mut MessageBuffer,
    base_path: &Path,
) -> Result<(), Box<dyn Error>> {
    match event {
        DiscordEventMessage::Ready { d } => {
            handle_ready(*d, dump_file, data_id, g_session_id, config, raw_text)?;
        }
        DiscordEventMessage::ReadySupplemental { .. } => {
            info!("READY_SUPPLEMENTAL received");
            if config.write_file.ready {
                dump_file
                    .ready_supplemental_json
                    .write_all(raw_text.as_bytes())?;
            }
        }
        DiscordEventMessage::Resumed { .. } => {
            info!("Session RESUMED");
        }
        other => {
            let handled = event_receive(
                other,
                data_id,
                last_channel_id,
                config,
                &mut dump_file.tracking_dump,
                message_buffer,
                base_path,
            )
            .await?;
            if !handled && config.write_file.todo_dump {
                writeln!(&mut dump_file.todo_dump, "{},\n", raw_text).ok();
            }
        }
    }
    Ok(())
}

fn handle_ready(
    d: receive_struct::ready::ReadyData,
    dump_file: &mut DumpFile,
    data_id: &mut DataId,
    g_session_id: &mut String,
    config: &Config,
    raw_text: &str,
) -> Result<(), Box<dyn Error>> {
    if let Some(name) = &d.user {
        let status = d
            .user_settings
            .as_ref()
            .map(|s| s.status.as_str())
            .unwrap_or("unknown");
        info!("Welcome {} {}", name.username, status);
    }
    if let Some(resume) = &d.resume_gateway_url {
        data_id.resume_gateway = resume.clone();
    }
    if config.write_file.ready {
        dump_file.ready_json.write_all(raw_text.as_bytes())?;
    }

    guild_mapping(
        &d,
        &mut data_id.guild_id,
        &mut data_id.channel_id,
        g_session_id,
        &mut dump_file.server_channel_dump,
        config,
    );

    if config.track_myself {
        get_username(&d, &mut data_id.username);
    }

    dm_mapping(
        &d,
        &mut data_id.dm_dump_hashmap,
        &mut dump_file.dm_dump_file,
        config,
    );
    muted_server_mapping(&d, &mut data_id.muted_server);

    if let Some(s_id) = d.sessions.first() {
        data_id.session_id = s_id.session_id.clone();
    }
    println!();

    d.sessions.iter().for_each(|s| {
        if let Some(data) = &s.client_info {
            if data.os != "other" {
                login_state!(format!("{} {}", data.client, data.os), s.status.as_str());
            }
        }
    });

    if config.print_muted_dm {
        for user_guild_setting in &d.user_guild_settings.entries {
            if user_guild_setting.guild_id.is_some() {
                continue;
            }
            for channel_override in &user_guild_setting.channel_overrides {
                let name = data_id
                    .dm_dump_hashmap
                    .get(&channel_override.channel_id)
                    .or_else(|| data_id.channel_id.get(&channel_override.channel_id))
                    .cloned()
                    .unwrap_or_else(|| String::from("Unknown"));

                if let Some(cfg) = &channel_override.mute_config {
                    if let Some(end) = &cfg.end_time {
                        if let Ok(date) = DateTime::parse_from_rfc3339(end) {
                            if date >= Local::now() {
                                info!("Muted: {} {} {}", channel_override.muted, date, name);
                            }
                        }
                    }
                } else {
                    info!("Muted: {} {}", channel_override.muted, name);
                }
            }
        }
    }
    Ok(())
}

async fn event_receive(
    msg: DiscordEventMessage,
    data_id: &mut DataId,
    last_channel_id: &mut Option<SnowflakeID>,
    cfg: &Config,
    tracking_dump: &mut File,
    message_buffer: &mut MessageBuffer,
    base_path: &Path,
) -> Result<bool, Box<dyn Error>> {
    match msg {
        DiscordEventMessage::MessageCreate { d } | DiscordEventMessage::MessageUpdate { d } => {
            handle_message(
                d,
                data_id,
                last_channel_id,
                cfg,
                tracking_dump,
                message_buffer,
                base_path,
            )
            .await?;
        }
        DiscordEventMessage::MessageDelete { d } => {
            handle_message_delete(d, data_id, cfg, message_buffer);
        }
        DiscordEventMessage::SessionsReplace { d } => {
            handle_sessions_replace(&d, cfg);
        }
        DiscordEventMessage::PresenceUpdate { d } => {
            handle_presence_update(&d, cfg);
        }
        DiscordEventMessage::UserSettingsUpdate { d } => {
            if cfg.event.user_settings_update {
                login_state!("UserSettingsUpdate", d.custom_status.as_str());
            }
        }
        DiscordEventMessage::CallCreate { d } => {
            handle_call(true, &d.channel_id, data_id, cfg);
        }
        DiscordEventMessage::CallDelete { d } => {
            handle_call(false, &d.channel_id, data_id, cfg);
        }
        DiscordEventMessage::TypingStart { d } => {
            handle_typing_start(&d, data_id, cfg);
        }
        DiscordEventMessage::VoiceStateUpdate { d } => {
            if cfg.event.voice_state_update {
                *last_channel_id = None;
                handle_voice_state_update(&d, data_id);
            }
        }
        DiscordEventMessage::UserGuildSettingsUpdate { d } => {
            handle_user_guild_settings(&d, data_id);
        }
        DiscordEventMessage::ChannelCreate { d } => {
            if let Some(guild) = data_id.guild_id.get(&d.guild_id) {
                info!("[Create channel] {guild} add > {}", d.name);
            }
        }
        DiscordEventMessage::ChannelDelete { d } => {
            if let Some(guild) = data_id.guild_id.get(&d.guild_id) {
                info!("[Delete channel] {guild} delete > {}", d.name);
            }
        }
        DiscordEventMessage::ChannelUpdate { d } => {
            if cfg.event.channel_update {
                if let Some(guild) = data_id.guild_id.get(&d.guild_id) {
                    info!("[Update channel] {guild} update > {}", d.name);
                }
            }
        }
        DiscordEventMessage::ChannelUnreadUpdate { d } => {
            if let Some(guild) = data_id.guild_id.get(&d.guild_id) {
                if cfg.debug {
                    info!(
                        "[Unread update] {guild} {} channel(s)",
                        d.channel_unread_updates.len()
                    );
                }
            }
        }
        DiscordEventMessage::GuildDelete { d } => {
            if let Some(guild) = data_id.guild_id.get(&d.id) {
                warn!("[Delete guild] {guild} delete");
            }
        }
        DiscordEventMessage::GuildMemberRemove { d } => {
            if let Some(guild) = data_id.guild_id.get(&d.guild_id) {
                warn!("[Delete member] {guild} delete > {}", d.user.username);
            }
        }
        DiscordEventMessage::GuildIntegrationsUpdate { d } => {
            if let Some(guild) = data_id.guild_id.get(&d.guild_id) {
                if cfg.debug {
                    info!("[GuildIntegrationsUpdate] {guild}");
                }
            }
        }
        DiscordEventMessage::IntegrationCreate { d }
        | DiscordEventMessage::IntegrationUpdate { d } => {
            if let Some(guild) = data_id.guild_id.get(&d.guild_id) {
                if cfg.debug {
                    let name = d.name.as_deref().unwrap_or("?");
                    info!("[Integration] {guild} > {name}");
                }
            }
        }
        DiscordEventMessage::MessageAck { d } => {
            if let Some(channel) = data_id.channel_id.get(&d.channel_id) {
                info!(
                    "[Message ack] > {} > {}",
                    channel,
                    message_buffer.get_message(d.message_id)
                );
            }
        }
        DiscordEventMessage::VoiceChannelStartTimeUpdate { d } => {
            if let Some(channel) = data_id.channel_id.get(&d.id) {
                if let Some(guild) = data_id.guild_id.get(&d.guild_id) {
                    match d.start_time {
                        Some(time) => info!(
                            "[Voice channel Connected to] > {} > {} > {}",
                            guild,
                            channel,
                            timestamp_to_date(time)
                        ),
                        None => info!(
                            "[Voice channel Disconnected from] > {} > {}",
                            guild, channel
                        ),
                    }
                }
            }
        }

        DiscordEventMessage::PassiveUpdateV1 { .. }
        | DiscordEventMessage::ChannelPinsAck { .. }
        | DiscordEventMessage::ChannelPinsUpdate { .. }
        | DiscordEventMessage::ChannelTopicUpdate { .. }
        | DiscordEventMessage::InteractionCreate { .. }
        | DiscordEventMessage::InteractionSuccess { .. }
        | DiscordEventMessage::Oauth2TokenCreate { .. }
        | DiscordEventMessage::UserSettingsProtoUpdate { .. }
        | DiscordEventMessage::UserApplicationUpdate { .. }
        | DiscordEventMessage::UserApplicationIdentityUpdate { .. }
        | DiscordEventMessage::UserConnectionsUpdate { .. }
        | DiscordEventMessage::UserUpdate { .. }
        | DiscordEventMessage::UserNoteUpdate { .. }
        | DiscordEventMessage::UserNonChannelAck { .. }
        | DiscordEventMessage::RecentMentionDelete { .. }
        | DiscordEventMessage::NotificationCenterItemCreate { .. }
        | DiscordEventMessage::NotificationCenterItemDelete { .. }
        | DiscordEventMessage::EmbeddedActivityUpdateV2 { .. }
        | DiscordEventMessage::ConversationSummaryUpdate { .. }
        | DiscordEventMessage::ContentInventoryInboxStale { .. }
        | DiscordEventMessage::MessageReactionAdd { .. }
        | DiscordEventMessage::MessageReactionRemove { .. }
        | DiscordEventMessage::MessageReactionRemoveAll { .. }
        | DiscordEventMessage::MessageReactionRemoveEmoji { .. }
        | DiscordEventMessage::MessageDeleteBulk { .. }
        | DiscordEventMessage::MessagePollVoteAdd { .. }
        | DiscordEventMessage::MessagePollVoteRemove { .. }
        | DiscordEventMessage::VoiceChannelEffectSend { .. }
        | DiscordEventMessage::VoiceServerUpdate { .. }
        | DiscordEventMessage::VoiceChannelStatusUpdate { .. }
        | DiscordEventMessage::RelationshipAdd { .. }
        | DiscordEventMessage::RelationshipRemove { .. }
        | DiscordEventMessage::WebhooksUpdate { .. }
        | DiscordEventMessage::GuildCreate { .. }
        | DiscordEventMessage::GuildUpdate { .. }
        | DiscordEventMessage::GuildMemberAdd { .. }
        | DiscordEventMessage::GuildMemberUpdate { .. }
        | DiscordEventMessage::GuildMemberChunk { .. }
        | DiscordEventMessage::GuildBanAdd { .. }
        | DiscordEventMessage::GuildRoleCreate { .. }
        | DiscordEventMessage::GuildRoleUpdate { .. }
        | DiscordEventMessage::GuildRoleDelete { .. }
        | DiscordEventMessage::GuildEmojisUpdate { .. }
        | DiscordEventMessage::GuildStickersUpdate { .. }
        | DiscordEventMessage::GuildApplicationCommandIndexUpdate { .. }
        | DiscordEventMessage::GuildAuditLogEntryCreate { .. }
        | DiscordEventMessage::GuildJoinRequestCreate { .. }
        | DiscordEventMessage::GuildJoinRequestUpdate { .. }
        | DiscordEventMessage::GuildScheduledEventCreate { .. }
        | DiscordEventMessage::GuildScheduledEventUpdate { .. }
        | DiscordEventMessage::GuildScheduledEventDelete { .. }
        | DiscordEventMessage::GuildScheduledEventUserAdd { .. }
        | DiscordEventMessage::GuildScheduledEventUserRemove { .. }
        | DiscordEventMessage::GuildScheduledEventExceptionsDelete { .. }
        | DiscordEventMessage::GuildSoundboardSoundCreate { .. }
        | DiscordEventMessage::GuildSoundboardSoundUpdate { .. }
        | DiscordEventMessage::GuildSoundboardSoundDelete { .. }
        | DiscordEventMessage::GuildSoundboardSoundsUpdate { .. }
        | DiscordEventMessage::SoundboardSounds { .. }
        | DiscordEventMessage::ThreadCreate { .. }
        | DiscordEventMessage::ThreadUpdate { .. }
        | DiscordEventMessage::ThreadDelete { .. }
        | DiscordEventMessage::ThreadMemberUpdate { .. }
        | DiscordEventMessage::ThreadMembersUpdate { .. }
        | DiscordEventMessage::ThreadListSync { .. }
        | DiscordEventMessage::StageInstanceCreate { .. }
        | DiscordEventMessage::StageInstanceUpdate { .. }
        | DiscordEventMessage::StageInstanceDelete { .. }
        | DiscordEventMessage::IntegrationDelete { .. }
        | DiscordEventMessage::InviteCreate { .. }
        | DiscordEventMessage::InviteDelete { .. }
        | DiscordEventMessage::SubscriptionCreate { .. }
        | DiscordEventMessage::SubscriptionUpdate { .. }
        | DiscordEventMessage::SubscriptionDelete { .. }
        | DiscordEventMessage::EntitlementCreate { .. }
        | DiscordEventMessage::EntitlementUpdate { .. }
        | DiscordEventMessage::EntitlementDelete { .. }
        | DiscordEventMessage::ApplicationCommandPermissionsUpdate { .. }
        | DiscordEventMessage::AutoModerationRuleCreate { .. }
        | DiscordEventMessage::AutoModerationRuleUpdate { .. }
        | DiscordEventMessage::AutoModerationRuleDelete { .. }
        | DiscordEventMessage::AutoModerationRuleExecution { .. } => {}

        _ => return Ok(false),
    }
    Ok(true)
}

async fn handle_message(
    mut d: receive_struct::message_create::MessageCreateData,
    data_id: &mut DataId,
    last_channel_id: &mut Option<SnowflakeID>,
    cfg: &Config,
    tracking_dump: &mut File,
    message_buffer: &mut MessageBuffer,
    base_path: &Path,
) -> Result<(), Box<dyn Error>> {
    if !cfg.event.message_create {
        return Ok(());
    }
    if d.guild_id.is_none() {
        let tracked = cfg.dm_channel_id_tracking.is_empty()
            || cfg
                .dm_channel_id_tracking
                .iter()
                .any(|x| d.channel_id == *x);
        if cfg.dm_track && tracked {
            dm_message(&mut d, data_id, base_path, cfg, message_buffer).await?;
        }
        *last_channel_id = None;
    } else if cfg.server_track {
        let next = *last_channel_id == Some(d.channel_id);
        normal_message(
            &mut d,
            data_id,
            tracking_dump,
            base_path,
            cfg,
            next,
            message_buffer,
        )
        .await?;
        *last_channel_id = Some(d.channel_id);
    }
    Ok(())
}

fn handle_message_delete(
    d: receive_struct::message_delete::MessageDeleteData,
    data_id: &DataId,
    cfg: &Config,
    message_buffer: &MessageBuffer,
) {
    if !cfg.event.message_delete {
        return;
    }
    if let Some(g_id) = d.guild_id.as_ref() {
        if let (Some(g_name), Some(ch_name)) = (
            data_id.guild_id.get(g_id),
            data_id.channel_id.get(&d.channel_id),
        ) {
            if cfg.server_track {
                dm_msg_delete!("\n[DELETE] {g_name} > {ch_name}");
                dm_msg_delete!("{}", message_buffer.get_message(d.id));
            }
        }
    } else if let Some(e) = data_id.dm_dump_hashmap.get(&d.channel_id) {
        if cfg.dm_track {
            dm_msg_delete!("\n[DELETE] [DM] > {e}");
            dm_msg_delete!("{}", message_buffer.get_message(d.id));
        }
    }
}

fn handle_sessions_replace(
    d: &[receive_struct::session_replace::SessionReplaceData],
    cfg: &Config,
) {
    if !cfg.event.session_replace {
        return;
    }
    println!("{}", "-".repeat(10));
    d.iter()
        .filter(|x| x.client_info.os != "other")
        .for_each(|x| {
            login_state!(
                format!("{} {}", x.client_info.client, x.client_info.os),
                x.status.as_str()
            );
        });
    println!("{}", "-".repeat(10));
}

fn handle_presence_update(d: &receive_struct::presence_update::PresenceData, cfg: &Config) {
    if !cfg.event.presence_update {
        return;
    }
    if let Some(name) = &d.user.username {
        if let Some(g_name) = &d.user.global_name {
            error!("{}", g_name);
        }
        error!("{}", name);
        login_state!("PresenceUpdate", d.status.as_str());
    }
}

fn handle_call(start: bool, channel_id: &SnowflakeID, data_id: &DataId, cfg: &Config) {
    let enabled = if start {
        cfg.event.call_dm_create
    } else {
        cfg.event.call_dm_delete
    };
    if !enabled {
        return;
    }
    let time = Local::now().format("%d/%m/%Y %H:%M:%S");
    let verb = if start { "Call start" } else { "Call end" };
    match data_id.dm_dump_hashmap.get(channel_id) {
        Some(channel) => info!("({time}) {verb} with: {channel}"),
        None => warn!("({time}) {verb} channel id NONE"),
    }
}

fn handle_typing_start(
    d: &receive_struct::typing_start::TypingData,
    data_id: &DataId,
    cfg: &Config,
) {
    if !cfg.event.typing_start {
        return;
    }
    if let Some(channel) = data_id.channel_id.get(&d.channel_id) {
        if let Some(g) = &d.guild_id {
            if let Some(guild) = data_id.guild_id.get(g) {
                println!("Typing: {} > {} > {}", guild, channel, d.user_id);
                return;
            }
        }
        println!("Typing: [DM] > {} > {}", channel, d.user_id);
    }
}

fn handle_voice_state_update(
    d: &receive_struct::voice_state_update::VoiceStateUpdateData,
    data_id: &DataId,
) {
    let member_id = d.member.as_ref().map(|m| &m.user.id).or(Some(&d.user_id));

    if let Some(member_id) = member_id {
        if !data_id.username.iter().any(|s| s == member_id) {
            return;
        }
        let Some(channel_id) = &d.channel_id else {
            voice_disconnect!("\nVoice: disconnected");
            return;
        };
        let Some(channel_name) = data_id
            .channel_id
            .get(channel_id)
            .or_else(|| data_id.dm_dump_hashmap.get(channel_id))
        else {
            return;
        };

        let context = match &d.guild_id {
            Some(guild_id) => match data_id.guild_id.get(guild_id) {
                Some(guild_name) => Some(format!("{} > {}", guild_name, channel_name)),
                None => None,
            },
            None => Some(format!("[DM] > {}", channel_name)),
        };

        if let Some(context) = context {
            let member_name = d
                .member
                .as_ref()
                .map(|m| m.user.username.as_str())
                .unwrap_or("Unknown");
            voice_connect!(
                "\nVoice: {}\n\t{} \x1B[0m[Mute: {}, Deaf: {}, SS: {}, Cam: {}]",
                context,
                member_name,
                bool_state!(d.self_mute),
                bool_state!(d.self_deaf),
                bool_state!(d.self_stream.unwrap_or(false)),
                bool_state!(d.self_video),
            );
        }
    }
}

fn handle_user_guild_settings(
    d: &receive_struct::user_guild_settings_update::UserGuildSettingsUpdateData,
    data_id: &DataId,
) {
    if d.guild_id.is_some() {
        return;
    }
    if d.channel_overrides.is_empty() {
        info!("Unmuted ? shrug");
        return;
    }
    for channel_override in &d.channel_overrides {
        info!(
            "Muted: {} {}",
            channel_override.muted,
            data_id
                .dm_dump_hashmap
                .get(&channel_override.channel_id)
                .map(String::as_str)
                .unwrap_or("Unknown")
        );
    }
}

async fn crash_discord(p0: &mut GatewaySink) {
    let crah = json!({
        "op": 2,
        "d": {
            "token": "Bot ",
            "properties": {
                "$os": "linux",
                "$browser": "Discord iOS",
                "$device": "Discord iOS"
            },
            "compress": true,
            "large_threshold": 250,
        }
    });

    let msg = match serde_json::to_string(&crah) {
        Ok(m) => m,
        Err(e) => {
            error!("crash payload serialize: {e}");
            return;
        }
    };
    println!("{}", msg);
    if let Err(err) = p0.send(tungstenite::Message::text(msg)).await {
        error!("Error sending payload: {:?}", err);
    }
}

fn serde_parsing_error(e: &serde_json::error::Error, msg_text: &str) {
    error!("ready test: {e}\n ->{msg_text} ");
    if let Some(captures) = utils::reg_parse_err().captures(&e.to_string()) {
        if let (Some(line), Some(column)) = (captures.get(1), captures.get(2)) {
            let Ok(line_num) = line.as_str().parse::<usize>() else {
                return;
            };
            let Ok(column_num) = column.as_str().parse::<usize>() else {
                return;
            };
            eprintln!(
                "Error parsing json: Line {}, Column {}",
                line_num, column_num
            );
            if column_num == 0 {
                eprintln!("Text: {}", msg_text);
            } else {
                let start_index = column_num.saturating_sub(25);
                let end_index = column_num.saturating_add(25).min(msg_text.len());
                eprintln!("{}", &msg_text[start_index..end_index]);
            }
        }
    }
}

macro_rules! log_change {
    ($tracking:expr, $name:expr, $old:expr, $new:expr) => {{
        warn!("{} change from {} to {}", $name, $old, $new);
        let _ = writeln!($tracking, "{} change from {} to {}", $name, $old, $new);
    }};
}

macro_rules! log_bool_change {
    ($tracking:expr, $name:expr, $e:expr) => {
        log_change!($tracking, $name, $e.0, $e.1)
    };
}

fn config_change(changed: Changed<Vec<ConfigChange>>, tracking: &mut File) {
    let Changed::Changed(changes) = changed else {
        return;
    };

    for change in changes {
        match change {
            ConfigChange::DmTrack(e) => log_bool_change!(tracking, "DmTrack", e),
            ConfigChange::ServerTrack(e) => log_bool_change!(tracking, "ServerTrack", e),
            ConfigChange::MessageBufferSize(e) => {
                log_bool_change!(tracking, "MessageBufferSize", e)
            }
            ConfigChange::Token(e) => log_bool_change!(tracking, "Token", e),
            ConfigChange::DmChannelIdTracking(items) => {
                for item in items {
                    match item {
                        VecChange::Added(_, e) => {
                            warn!("UserIdTracking Added {:?}", e);
                            let _ = writeln!(tracking, "UserIdTracking change to {e:?}");
                        }
                        VecChange::Changed(_, e) => {
                            warn!("UserIdTracking Changed {:?}", e);
                            let _ = writeln!(tracking, "UserIdTracking change from {:?}", e);
                        }
                        VecChange::Removed(_, e) => {
                            warn!("UserIdTracking Removed {:?}", e);
                            let _ = writeln!(tracking, "UserIdTracking change to {e:?}");
                        }
                    }
                }
            }
            ConfigChange::TrackMyself(e) => log_bool_change!(tracking, "TrackMyself", e),
            ConfigChange::Event(items) => {
                for item in items {
                    match item {
                        EventChange::ChannelUpdate(e) => {
                            log_bool_change!(tracking, "ChannelUpdate", e)
                        }
                        EventChange::MessageCreate(e) => {
                            log_bool_change!(tracking, "MessageCreate", e)
                        }
                        EventChange::MessageDelete(e) => {
                            log_bool_change!(tracking, "MessageDelete", e)
                        }
                        EventChange::SessionReplace(e) => {
                            log_bool_change!(tracking, "SessionReplace", e)
                        }
                        EventChange::PresenceUpdate(e) => {
                            log_bool_change!(tracking, "PresenceUpdate", e)
                        }
                        EventChange::UserSettingsUpdate(e) => {
                            log_bool_change!(tracking, "UserSettingsUpdate", e)
                        }
                        EventChange::CallDmCreate(e) => {
                            log_bool_change!(tracking, "CallDmCreate", e)
                        }
                        EventChange::CallDmDelete(e) => {
                            log_bool_change!(tracking, "CallDmDelete", e)
                        }
                        EventChange::TypingStart(e) => log_bool_change!(tracking, "TypingStart", e),
                        EventChange::VoiceStateUpdate(e) => {
                            log_bool_change!(tracking, "VoiceStateUpdate", e)
                        }
                    }
                }
            }
            ConfigChange::DownloadMedia(e) => log_bool_change!(tracking, "DownloadMedia", e),
            ConfigChange::WriteFile(items) => {
                for item in items {
                    match item {
                        WriteFileChange::Ready(e) => log_bool_change!(tracking, "Ready", e),
                        WriteFileChange::ServerChannel(e) => {
                            log_bool_change!(tracking, "ServerChannel", e)
                        }
                        WriteFileChange::DmChannel(e) => log_bool_change!(tracking, "DmChannel", e),
                        WriteFileChange::TodoDump(e) => log_bool_change!(tracking, "TodoDump", e),
                        WriteFileChange::Tracking(e) => log_bool_change!(tracking, "Tracking", e),
                    }
                }
            }
            ConfigChange::PrintMutedDm(e) => log_bool_change!(tracking, "Print Muted DM", e),
            ConfigChange::Debug(e) => log_bool_change!(tracking, "Print Debug", e),
        }
    }
}

struct DumpFile {
    ready_json: File,
    ready_supplemental_json: File,
    server_channel_dump: File,
    dm_dump_file: File,
    tracking_dump: File,
    todo_dump: File,
}

struct DataId {
    guild_id: HashMap<SnowflakeID, String>,
    channel_id: HashMap<SnowflakeID, String>,
    dm_dump_hashmap: HashMap<SnowflakeID, String>,
    muted_server: HashMap<SnowflakeID, String>,
    username: Vec<SnowflakeID>,
    session_id: String,
    resume_gateway: String,
}

impl DataId {
    fn clear(&mut self) {
        self.guild_id.clear();
        self.channel_id.clear();
        self.dm_dump_hashmap.clear();
        self.muted_server.clear();
        self.username.clear();
        self.session_id.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::receive_struct::receive_event::DiscordEventMessage;

    #[test]
    fn parse_real_ready_dump() {
        use super::receive_struct::ready::{ReadyData, UserGuildSettingsContainer};
        let Ok(s) = std::fs::read_to_string("/tmp/ready_event.json") else {
            eprintln!("skipped (no /tmp/ready_event.json)");
            return;
        };
        let val: serde_json::Value = serde_json::from_str(&s).expect("envelope as Value");
        let d = val.get("d").expect("d field");

        let ugs_val = d.get("user_guild_settings").expect("ugs");
        let ugs_str = serde_json::to_string(ugs_val).unwrap();
        let mut de = serde_json::Deserializer::from_str(&ugs_str);
        let ugs: Result<UserGuildSettingsContainer, _> = serde_path_to_error::deserialize(&mut de);
        if let Err(e) = ugs {
            panic!("UGS isolated parse failed at `{}`: {}", e.path(), e.inner());
        }

        let d_str = serde_json::to_string(d).unwrap();
        let mut de = serde_json::Deserializer::from_str(&d_str);
        let r: Result<ReadyData, _> = serde_path_to_error::deserialize(&mut de);
        match r {
            Ok(_) => {}
            Err(e) => panic!("ReadyData parse failed at `{}`: {}", e.path(), e.inner()),
        }
    }

    #[test]
    fn parse_every_todo_dump_event() {
        let Ok(s) = std::fs::read_to_string("todo_dump.txt") else {
            eprintln!("skipped (no todo_dump.txt)");
            return;
        };
        let mut i = 0usize;
        let mut count = 0usize;
        let mut failures: Vec<String> = Vec::new();
        while i < s.len() {
            while i < s.len() && matches!(s.as_bytes()[i], b' ' | b'\n' | b'\r' | b'\t' | b',') {
                i += 1;
            }
            if i >= s.len() {
                break;
            }
            let mut de =
                serde_json::Deserializer::from_str(&s[i..]).into_iter::<serde_json::Value>();
            let val = match de.next() {
                Some(Ok(v)) => v,
                _ => break,
            };
            i += de.byte_offset();

            let t = val
                .get("t")
                .and_then(|v| v.as_str())
                .unwrap_or("<no t>")
                .to_string();
            let raw = serde_json::to_string(&val).unwrap();
            let mut ed = serde_json::Deserializer::from_str(&raw);
            let parsed: Result<DiscordEventMessage, _> = serde_path_to_error::deserialize(&mut ed);
            match parsed {
                Ok(_) => count += 1,
                Err(e) => failures.push(format!("{t}: at `{}` -> {}", e.path(), e.inner())),
            }
        }
        if !failures.is_empty() {
            panic!(
                "parsed {} events ok but {} failed:\n{}",
                count,
                failures.len(),
                failures.join("\n")
            );
        }
        eprintln!("parsed {} events from todo_dump.txt", count);
    }
}
