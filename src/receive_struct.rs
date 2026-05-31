use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::{fmt::Write as _, io::Write as _};

use chrono::Local;
use viuer::print;

use crate::config::config::Config;
use crate::mapping_struct::{DMMapping, ServerChannel, ServerMapping};
use crate::message_buffer::MessageBuffer;
use crate::receive_struct::message_create::MessageCreateData;
use crate::receive_struct::ready::ReadyData;
use crate::utils::{http_client, parse_timestamp, reg_content, reg_media, SnowflakeID};
use crate::{dm_msg, error, info, msg, my_msg, serv_msg, DataId};

mod call_create;
mod call_delete;
mod channel_create;
pub mod channel_unread_update;
mod channel_update;
pub mod gateway;
pub mod gateway_events;
mod guild_delete;
pub mod guild_integration_update;
mod guild_member_remove;
pub mod hello;
pub mod integration_update;
mod message_ack;
pub mod message_create;
pub mod message_delete;
mod msg_response;
pub mod presence_update;
pub mod ready;
pub mod receive_event;
pub mod session_replace;
pub mod typing_start;
mod user_application_identity_update;
pub mod user_guild_settings_update;
pub mod user_settings_update;
mod voice_channel_start_time_update;
mod voice_channel_status_update;
mod voice_server_update;
pub mod voice_state_update;

pub async fn dm_message(
    msg: &mut MessageCreateData,
    data_id: &mut DataId,
    base_path: &Path,
    cfg: &Config,
    message_buffer: &mut MessageBuffer,
) -> Result<(), Box<dyn Error>> {
    let channel_id = msg.channel_id;

    let mut name = String::new();
    if let Some(dm) = data_id.dm_dump_hashmap.get(&channel_id) {
        name.push_str(dm);
    } else {
        data_id
            .dm_dump_hashmap
            .insert(channel_id, msg.author.username.clone());
        name.push_str(&msg.author.username);
    }

    if msg.call.is_some() {
        split_dm_log(base_path, &name, "[DM CALL START]")?;
        return Ok(());
    }

    let header_message = format!("\n[DM] > {}", name);
    split_dm_log(base_path, &name, &header_message)?;
    dm_msg!("{}", header_message);

    let attachment = get_all_attachments(msg, base_path, cfg).await;
    let reply = get_reply_to(msg);

    if let Some(t) = type_filed(msg.type_field) {
        msg.content = t;
    }

    let msg_flags = if msg.flags == 4096 { "[Muted] " } else { "" };
    let edited = if msg.edited_timestamp.is_none() {
        " "
    } else {
        " [Update] "
    };
    let timestamp = parse_timestamp(&msg.timestamp);
    let msg_build = format!(
        "\t{}{}({}) {}:{}{}{}",
        msg_flags, reply, timestamp, msg.author.username, edited, msg.content, attachment,
    );

    split_dm_log(base_path, &name, &format!("     {}", msg_build))?;
    dm_msg!("{}", msg_build);

    if !attachment.is_empty() {
        image_in_terminal(&attachment).await;
    }

    message_buffer.add_message(msg.id, msg_build);
    Ok(())
}

fn split_dm_log(base_path: &Path, name: &str, msg: &str) -> Result<(), Box<dyn Error>> {
    let safe_name = if name.is_empty() { "unknown" } else { name };
    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(base_path.join(format!("dm/{}.txt", safe_name)))?;
    file.write_all(msg.as_bytes())?;
    Ok(())
}

pub async fn normal_message(
    msg: &mut MessageCreateData,
    data_id: &DataId,
    tracking_dump: &mut File,
    base_path: &Path,
    cfg: &Config,
    new_msg: bool,
    message_buffer: &mut MessageBuffer,
) -> Result<(), Box<dyn Error>> {
    let attachment = get_all_attachments(msg, base_path, cfg).await;
    let reply = get_reply_to(msg);
    let mut serv_build = String::new();

    if !new_msg {
        let guild_id = msg.guild_id.unwrap_or_default();
        let channel_id = msg.channel_id;

        let muted_server_guild = data_id
            .muted_server
            .get(&guild_id)
            .map(String::as_str)
            .unwrap_or("");
        let guild_name = data_id
            .guild_id
            .get(&guild_id)
            .map(String::as_str)
            .unwrap_or("no guild id");
        let muted_server_channel = data_id
            .muted_server
            .get(&channel_id)
            .map(String::as_str)
            .unwrap_or("");
        let channel_name = data_id
            .channel_id
            .get(&channel_id)
            .map(String::as_str)
            .unwrap_or("no channel id");

        serv_build = format!(
            "\n{} {} > {} {}",
            muted_server_guild, guild_name, muted_server_channel, channel_name
        );
        serv_msg!("{}", serv_build);
    }

    if let Some(t) = type_filed(msg.type_field) {
        msg.content = t;
    }

    if let Some(inter) = &msg.interaction
        && msg.content.is_empty()
    {
        msg.content = format!(" {} -> Ask [{}]", inter.user.username, inter.name);
    }

    let bot_label = if msg.author.bot == Some(true) {
        " [BOT]"
    } else {
        ""
    };
    let edited = if msg.edited_timestamp.is_none() {
        " "
    } else {
        " [Update] "
    };

    let msg_build = format!(
        "\t{}({}) {}{}:{}{}{}",
        reply,
        parse_timestamp(&msg.timestamp),
        msg.author.username,
        bot_label,
        edited,
        msg.content,
        attachment
    );

    if data_id.username.iter().any(|i| i == &msg.author.id) {
        writeln!(tracking_dump, "{}", serv_build)?;
        writeln!(tracking_dump, "     {}", msg_build)?;
        my_msg!("{}", msg_build);
    } else {
        msg!("{}", msg_build);
    }

    if !attachment.is_empty() {
        image_in_terminal(&attachment).await;
    }

    message_buffer.add_message(msg.id, msg_build);
    Ok(())
}

fn is_discord_media(url: &str) -> bool {
    url.contains("https://cdn.discordapp.com") || url.contains("https://media.discordapp.net")
}

pub async fn get_all_attachments(
    msg: &MessageCreateData,
    base_path: &Path,
    cfg: &Config,
) -> String {
    let mut data = String::new();
    let safe_author = msg.author.username.replace('.', "_");

    if let Some(cap) = reg_content().captures(&msg.content) {
        if let (Some(url), Some(name)) = (cap.get(0), cap.get(1)) {
            if cfg.download_media {
                dl_media(url.as_str(), name.as_str(), &safe_author, base_path)
                    .await
                    .ok();
            }
            write!(data, "\n\t direct: {}", url.as_str()).ok();
        }
    }

    for attachment in &msg.attachments {
        if let Some(url) = &attachment.url {
            if is_discord_media(url) && cfg.download_media {
                dl_media(url, &attachment.filename, &safe_author, base_path)
                    .await
                    .ok();
            }
            write!(data, "\n\t attachments: {}", url).ok();
        }
    }

    for embed in &msg.embeds {
        if let Some(url) = embed.image.as_ref().and_then(|img| img.url.as_ref()) {
            if is_discord_media(url) && cfg.download_media {
                if let Some(name) = reg_media().captures(url).and_then(|cap| cap.get(1)) {
                    dl_media(url, name.as_str(), &safe_author, base_path)
                        .await
                        .ok();
                }
            }
            write!(data, "\n\t embeds image: {}", url).ok();
        }

        if let Some(url) = embed.video.as_ref().and_then(|vid| vid.url.as_ref()) {
            write!(data, "\n\t embeds video: {}", url).ok();
        }

        if let Some(fields) = &embed.fields {
            for field in fields {
                write!(data, "\n\t embeds fields: {}: {}", field.name, field.value).ok();
            }
        }
        if let Some(desc) = &embed.description {
            write!(data, "\n\t embeds fields: {}", desc).ok();
        }
    }

    if let Some(stickers) = &msg.sticker_items {
        for sticker in stickers {
            write!(data, "\n\t Stickers: {}", sticker.name).ok();
        }
    }

    data
}

pub async fn image_in_terminal(url: &str) {
    if url.is_empty() {
        return;
    }
    for url in url.split(": ").filter(|s| s.contains("http")) {
        let Ok(response) = http_client().get(url).send().await else {
            continue;
        };
        let Ok(bytes) = response.bytes().await else {
            continue;
        };
        if let Ok(image) = image::load_from_memory(&bytes) {
            let _ = print(
                &image,
                &viuer::Config {
                    absolute_offset: false,
                    x: 4,
                    height: Some(15),
                    ..Default::default()
                },
            );
        }
    }
}

pub fn get_reply_to(msg: &MessageCreateData) -> String {
    let mut st = String::new();
    if let Some(m) = &msg.referenced_message {
        st.push_str(&format!(
            "[Reply] {}: {}\n\t\t└─",
            m.author.username, m.content
        ))
    }
    st
}

pub fn muted_server_mapping(e: &ReadyData, muted_channel: &mut HashMap<SnowflakeID, String>) {
    for x in &e.user_guild_settings.entries {
        if let Some(gid) = &x.guild_id {
            muted_channel.insert(
                *gid,
                if x.muted {
                    "[MUTED]".to_string()
                } else if x.message_notifications == 1 {
                    "[MUTED Notif]".to_string()
                } else {
                    String::new()
                },
            );
        }
        for override_ in &x.channel_overrides {
            muted_channel.insert(
                override_.channel_id,
                if override_.muted {
                    "MUTED".to_string()
                } else if override_.message_notifications == 1 {
                    "MUTED Notif".to_string()
                } else {
                    String::new()
                },
            );
        }
    }
}

pub fn get_username(e: &ReadyData, username: &mut Vec<SnowflakeID>) {
    if let Some(user) = &e.user {
        username.push(user.id);
    }
}

pub fn guild_mapping(
    e: &ReadyData,
    guild_id: &mut HashMap<SnowflakeID, String>,
    channel_id: &mut HashMap<SnowflakeID, String>,
    session_id: &mut String,
    server_mapping: &mut File,
    cfg: &Config,
) {
    if let Some(id) = &e.session_id {
        *session_id = id.clone();
    }

    let mut g_vec = Vec::with_capacity(e.guilds.len());

    for x in &e.guilds {
        let mut server_channels = Vec::with_capacity(x.channels.len() + x.threads.len());

        for channel in &x.channels {
            server_channels.push(ServerChannel {
                channel_type: channel_type(&channel.type_field),
                channel_id: channel.id,
                channel_name: channel.name.clone(),
            });
            channel_id.insert(channel.id, channel.name.clone());
        }

        for thread in &x.threads {
            let labelled = format!("Thread: {}", thread.name);
            server_channels.push(ServerChannel {
                channel_type: channel_type(&thread.type_field),
                channel_id: thread.id,
                channel_name: labelled.clone(),
            });
            channel_id.insert(thread.id, labelled);
        }

        let name = x.display_name().to_string();
        guild_id.insert(x.id, name.clone());
        g_vec.push(ServerMapping {
            size: x.member_count as u16,
            server_id: x.id,
            server_name: name,
            server_channels,
        });
    }

    if cfg.write_file.server_channel {
        let mut writer = BufWriter::new(server_mapping);
        if let Err(e) = serde_json::to_writer(&mut writer, &g_vec) {
            error!("Failed to write server mapping to file: {}", e);
        }
        if let Err(e) = writer.flush() {
            error!("Failed to flush server mapping: {}", e);
        }
    }

    info!("Server found {}", guild_id.len());
    info!("Channel mapped {}", channel_id.len());
}

pub fn dm_mapping(
    e: &ReadyData,
    dm_mapping: &mut HashMap<SnowflakeID, String>,
    dm_dump_file: &mut File,
    config: &Config,
) {
    let Some(dms) = &e.private_channels else {
        info!("DM found 0");
        return;
    };

    let user_table: HashMap<&str, &str> = e
        .users
        .iter()
        .map(|u| (u.id.as_str(), u.username.as_str()))
        .collect();

    let mut dm_map = Vec::with_capacity(dms.len());

    for dm in dms {
        let mut ppl = String::from("[");

        if !dm.recipients.is_empty() {
            for recipient in &dm.recipients {
                ppl.push_str(&recipient.username);
                ppl.push_str(", ");
            }
        } else {
            for rid in &dm.recipient_ids {
                let uname = user_table
                    .get(rid.as_str())
                    .copied()
                    .unwrap_or(rid.as_str());
                ppl.push_str(uname);
                ppl.push_str(", ");
            }
        }

        if let Some(my_name) = &e.user {
            ppl.push_str(&my_name.username);
            ppl.push_str("] ");
            if let Some(name) = &dm.name {
                ppl.push_str(name);
            }
        } else {
            ppl.push(']');
        }

        dm_mapping.insert(dm.id, ppl);

        let participants: Vec<crate::receive_struct::ready::Recipient> =
            if !dm.recipients.is_empty() {
                dm.recipients.clone()
            } else {
                dm.recipient_ids
                    .iter()
                    .map(|rid| crate::receive_struct::ready::Recipient {
                        id: rid.clone(),
                        username: user_table
                            .get(rid.as_str())
                            .copied()
                            .unwrap_or("")
                            .to_string(),
                        ..Default::default()
                    })
                    .collect()
            };

        dm_map.push(DMMapping {
            channel_id: dm.id,
            channel_name: dm.name.clone(),
            participant: participants,
        });
    }

    if config.write_file.dm_channel {
        let mut writer = BufWriter::new(dm_dump_file);
        if let Err(e) = serde_json::to_writer(&mut writer, &dm_map) {
            error!("Failed to write DM mapping to file: {}", e);
        }
        if let Err(e) = writer.flush() {
            error!("Failed to flush DM mapping: {}", e);
        }
    }

    info!("DM found {}", dm_mapping.len());
}

fn type_filed(tf: u8) -> Option<String> {
    match tf {
        6 => Some(String::from("[PIN MSG]")),
        7 => Some(String::from("[JOIN SERVER]")),
        8 => Some(String::from("[BOOST SERVER]")),
        _ => None,
    }
}

fn channel_type(x1: &i64) -> String {
    match x1 {
        0 => "Text".to_string(),
        2 => "Voice".to_string(),
        4 => "Category".to_string(),
        5 => "Announce".to_string(),
        11 => "Thread".to_string(),
        13 => "Stage".to_string(),
        15 => "Forum".to_string(),
        16 => "Media".to_string(),
        n => n.to_string(),
    }
}

async fn dl_media(
    url: &str,
    final_name: &str,
    name: &str,
    base_path: &Path,
) -> Result<(), Box<dyn Error>> {
    let path = base_path.join(format!("img/{}", name));
    fs::create_dir_all(&path)?;

    let data = http_client().get(url).send().await?.bytes().await?;

    let out = path.join(format!(
        "{}_{}",
        Local::now().format("%d_%m_%Y_%H_%M_%S_%6f"),
        final_name,
    ));
    let mut save_file = File::create(out)?;
    save_file.write_all(&data)?;
    Ok(())
}
