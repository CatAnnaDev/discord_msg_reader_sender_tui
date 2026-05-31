use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use tokio::sync::RwLock;

use crate::utils::SnowflakeID;

pub type SharedState = Arc<RwLock<AppState>>;

fn yn(b: bool) -> String {
    if b { "on".into() } else { "off".into() }
}

#[derive(Clone, Debug)]
pub struct GuildSummary {
    pub id: SnowflakeID,
    pub name: String,

    pub icon_url: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PresenceStatus {
    Online,
    Idle,
    Dnd,
    #[default]
    Offline,
}

impl PresenceStatus {
    pub fn parse(s: &str) -> Self {
        match s {
            "online" => Self::Online,
            "idle" => Self::Idle,
            "dnd" => Self::Dnd,
            _ => Self::Offline,
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Self::Online => "●",
            Self::Idle => "◐",
            Self::Dnd => "⏼",
            Self::Offline => "○",
        }
    }
}

pub const TYPING_TTL: Duration = Duration::from_secs(9);

#[derive(Clone, Debug)]
pub struct ChannelSummary {
    pub id: SnowflakeID,
    pub name: String,

    pub recipient_id: Option<SnowflakeID>,

    pub kind: i64,
    pub parent_id: Option<SnowflakeID>,
    pub position: i64,

    pub last_activity: u64,

    pub avatar_url: Option<String>,
}

pub const DM_GUILD_ID: SnowflakeID = SnowflakeID::const_zero();

#[derive(Clone, Debug)]
pub struct UiPrefs {
    pub toast_enabled: bool,
    pub toast_secs: u64,
    pub notify_mentions: bool,
    pub show_relative_time: bool,
    pub show_typing: bool,
    pub dsp_rx: bool,
    pub dsp_tx: bool,
    pub self_mute: bool,
    pub self_deaf: bool,
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            toast_enabled: true,
            toast_secs: 6,
            notify_mentions: true,
            show_relative_time: true,
            show_typing: true,
            dsp_rx: true,
            dsp_tx: true,
            self_mute: false,
            self_deaf: false,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct EditCfg {
    pub download_media: bool,
    pub debug: bool,
    pub print_muted_dm: bool,
    pub dm_track: bool,
    pub server_track: bool,
    pub track_myself: bool,
    pub message_buffer_size: usize,
    pub voice_input_device: Option<String>,
    pub voice_output_device: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingKind {
    Bool,
    U64,
    Text,
}

pub struct SettingRow {
    pub label: &'static str,
    pub kind: SettingKind,
}

pub const SETTINGS: &[SettingRow] = &[
    SettingRow { label: "Desktop toasts (DM popups)", kind: SettingKind::Bool },
    SettingRow { label: "Toast duration (seconds)", kind: SettingKind::U64 },
    SettingRow { label: "Notify on @mentions (servers)", kind: SettingKind::Bool },
    SettingRow { label: "Relative timestamps", kind: SettingKind::Bool },
    SettingRow { label: "Show typing indicators", kind: SettingKind::Bool },
    SettingRow { label: "Download media", kind: SettingKind::Bool },
    SettingRow { label: "Debug logging", kind: SettingKind::Bool },
    SettingRow { label: "Show muted DMs", kind: SettingKind::Bool },
    SettingRow { label: "Track DMs", kind: SettingKind::Bool },
    SettingRow { label: "Track servers", kind: SettingKind::Bool },
    SettingRow { label: "Track myself", kind: SettingKind::Bool },
    SettingRow { label: "Message buffer size", kind: SettingKind::U64 },
    SettingRow { label: "Voice input device", kind: SettingKind::Text },
    SettingRow { label: "Voice output device", kind: SettingKind::Text },
    SettingRow { label: "RX audio filters (HPF/gate/comp/AGC)", kind: SettingKind::Bool },
    SettingRow { label: "TX audio filters (HPF/gate/comp/AGC)", kind: SettingKind::Bool },
];

pub const TOAST_TTL: Duration = Duration::from_secs(6);

#[derive(Clone, Debug)]
pub struct Toast {
    pub title: String,
    pub body: String,
    pub at: Instant,
}

#[derive(Clone, Debug, Default)]
pub struct VoiceMember {
    pub id: SnowflakeID,
    pub name: String,
    pub mute: bool,
    pub deaf: bool,
    pub video: bool,
    pub stream: bool,
}

#[derive(Clone, Debug)]
pub struct SearchHit {
    pub icon: char,
    pub label: String,
    pub sub: String,
    pub guild: Option<SnowflakeID>,
    pub channel: Option<SnowflakeID>,
}

impl ChannelSummary {
    pub fn is_textual(&self) -> bool {
        matches!(self.kind, 0 | 1 | 3 | 5 | 10 | 11 | 12 | 15)
    }
}

#[derive(Clone, Debug)]
pub struct DisplayMessage {
    pub id: SnowflakeID,
    pub channel_id: SnowflakeID,
    pub author: String,
    pub author_id: SnowflakeID,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub edited: bool,
    pub is_self: bool,

    pub attachment_images: Vec<(SnowflakeID, String)>,
    pub reply_to: Option<(String, String)>,
    pub reactions: Vec<(String, u32)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Focus {
    #[default]
    ServerBar,
    Channels,
    Messages,
    Input,
}

impl Focus {
    pub fn next(self) -> Self {
        match self {
            Focus::ServerBar => Focus::Channels,
            Focus::Channels => Focus::Messages,
            Focus::Messages => Focus::Input,
            Focus::Input => Focus::ServerBar,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Focus::ServerBar => Focus::Input,
            Focus::Channels => Focus::ServerBar,
            Focus::Messages => Focus::Channels,
            Focus::Input => Focus::Messages,
        }
    }
}

#[derive(Default, Clone)]
pub struct PendingStream {
    pub uid: u64,
    pub token: Option<String>,
    pub endpoint: Option<String>,
    pub server: Option<String>,
}

#[derive(Default)]
pub struct AppState {
    pub guilds: Vec<GuildSummary>,

    pub channels_by_guild: HashMap<SnowflakeID, Vec<ChannelSummary>>,
    pub messages: HashMap<SnowflakeID, Vec<DisplayMessage>>,

    pub current_guild: Option<SnowflakeID>,
    pub current_channel: Option<SnowflakeID>,
    pub focus: Focus,

    pub guild_cursor: usize,
    pub channel_cursor: usize,
    pub message_cursor: usize,
    pub message_scroll: usize,
    pub input: String,
    pub input_history: Vec<String>,
    pub history_pos: Option<usize>,
    pub self_mute: bool,
    pub self_deaf: bool,
    pub status_line: String,
    pub should_quit: bool,
    pub channels_dirty: bool,

    pub my_user_id: Option<SnowflakeID>,
    pub my_username: String,

    pub image_cache: HashMap<SnowflakeID, image::DynamicImage>,

    pub preview_image: Option<SnowflakeID>,

    pub watching_streams: Vec<SnowflakeID>,
    pub big_streams: HashSet<SnowflakeID>,

    pub show_logs: bool,

    pub log_scroll: usize,

    pub joining_voice_token: Option<String>,
    pub joining_voice_endpoint: Option<String>,
    pub joining_voice_session: Option<String>,
    pub joining_voice_guild: Option<SnowflakeID>,
    pub voice_channel: Option<SnowflakeID>,
    pub voice_status: String,

    pub pending_streams: HashMap<String, PendingStream>,
    pub stream_conns: HashMap<
        u64,
        tokio::sync::mpsc::UnboundedSender<crate::voice::VoiceCommand>,
    >,
    pub golive_key: Option<String>,
    pub golive_cmd_tx:
        Option<tokio::sync::mpsc::UnboundedSender<crate::voice::VoiceCommand>>,

    pub image_backend: String,

    pub voice_cmd_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::voice::VoiceCommand>>,

    pub gateway_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::tui::ingest::GatewayAction>>,

    pub voice_input_device: Option<String>,
    pub voice_output_device: Option<String>,

    pub unread: HashSet<SnowflakeID>,

    pub voice_members: HashMap<SnowflakeID, Vec<VoiceMember>>,

    pub user_dir: HashMap<SnowflakeID, String>,

    pub presence: HashMap<SnowflakeID, PresenceStatus>,

    pub typing: HashMap<SnowflakeID, Vec<(SnowflakeID, Instant)>>,

    pub search_open: bool,
    pub search_input: String,
    pub search_cursor: usize,
    pub search_results: Vec<SearchHit>,

    pub editing_message: Option<SnowflakeID>,
    pub replying_to: Option<SnowflakeID>,
    pub unread_count: HashMap<SnowflakeID, u32>,
    pub conn_status: String,

    pub notifications: Vec<Toast>,

    pub guild_members: HashMap<SnowflakeID, HashSet<SnowflakeID>>,
    pub members_requested: HashSet<SnowflakeID>,
    pub show_help: bool,
    pub show_members: bool,
    pub settings_open: bool,
    pub settings_cursor: usize,
    pub settings_editing: bool,
    pub settings_buf: String,
    pub prefs: UiPrefs,
    pub edit_cfg: EditCfg,
    pub config_path: String,
}

impl AppState {
    pub fn new_shared() -> SharedState {
        Arc::new(RwLock::new(AppState::default()))
    }

    pub fn display_name(&self, id: SnowflakeID) -> String {
        self.user_dir
            .get(&id)
            .cloned()
            .unwrap_or_else(|| id.to_string())
    }

    pub fn note_user(&mut self, id: SnowflakeID, global: Option<&str>, username: &str) {
        let name = global
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .or_else(|| Some(username.trim()).filter(|s| !s.is_empty()));
        if let Some(name) = name {
            self.user_dir.insert(id, name.to_string());
        }
    }

    pub fn typing_names(&self, channel: SnowflakeID) -> Vec<String> {
        let me = self.my_user_id;
        self.typing
            .get(&channel)
            .map(|v| {
                v.iter()
                    .filter(|(u, t)| Some(*u) != me && t.elapsed() < TYPING_TTL)
                    .map(|(u, _)| self.display_name(*u))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn presence_of(&self, id: SnowflakeID) -> PresenceStatus {
        self.presence.get(&id).copied().unwrap_or_default()
    }

    fn toast_ttl(&self) -> Duration {
        Duration::from_secs(self.prefs.toast_secs.max(1))
    }

    pub fn push_toast(&mut self, title: impl Into<String>, body: impl Into<String>) {
        if !self.prefs.toast_enabled {
            return;
        }
        let ttl = self.toast_ttl();
        self.notifications.retain(|t| t.at.elapsed() < ttl);
        self.notifications.push(Toast {
            title: title.into(),
            body: body.into(),
            at: Instant::now(),
        });
        let n = self.notifications.len();
        if n > 5 {
            self.notifications.drain(0..n - 5);
        }
    }

    pub fn active_toasts(&self) -> Vec<&Toast> {
        let ttl = self.toast_ttl();
        self.notifications
            .iter()
            .filter(|t| t.at.elapsed() < ttl)
            .rev()
            .take(3)
            .collect()
    }

    pub fn setting_display(&self, idx: usize) -> String {
        let p = &self.prefs;
        let c = &self.edit_cfg;
        match idx {
            0 => yn(p.toast_enabled),
            1 => p.toast_secs.to_string(),
            2 => yn(p.notify_mentions),
            3 => yn(p.show_relative_time),
            4 => yn(p.show_typing),
            5 => yn(c.download_media),
            6 => yn(c.debug),
            7 => yn(c.print_muted_dm),
            8 => yn(c.dm_track),
            9 => yn(c.server_track),
            10 => yn(c.track_myself),
            11 => c.message_buffer_size.to_string(),
            12 => c.voice_input_device.clone().unwrap_or_else(|| "(default)".into()),
            13 => c.voice_output_device.clone().unwrap_or_else(|| "(default)".into()),
            14 => yn(p.dsp_rx),
            15 => yn(p.dsp_tx),
            _ => String::new(),
        }
    }

    pub fn setting_adjust(&mut self, idx: usize, delta: i64) {
        let toggle = |b: &mut bool| *b = !*b;
        match idx {
            0 => toggle(&mut self.prefs.toast_enabled),
            1 => {
                let v = self.prefs.toast_secs as i64 + delta;
                self.prefs.toast_secs = v.clamp(1, 60) as u64;
            }
            2 => toggle(&mut self.prefs.notify_mentions),
            3 => toggle(&mut self.prefs.show_relative_time),
            4 => toggle(&mut self.prefs.show_typing),
            5 => toggle(&mut self.edit_cfg.download_media),
            6 => toggle(&mut self.edit_cfg.debug),
            7 => toggle(&mut self.edit_cfg.print_muted_dm),
            8 => toggle(&mut self.edit_cfg.dm_track),
            9 => toggle(&mut self.edit_cfg.server_track),
            10 => toggle(&mut self.edit_cfg.track_myself),
            11 => {
                let v = self.edit_cfg.message_buffer_size as i64 + delta * 50;
                self.edit_cfg.message_buffer_size = v.clamp(50, 10_000) as usize;
            }
            14 => toggle(&mut self.prefs.dsp_rx),
            15 => toggle(&mut self.prefs.dsp_tx),
            _ => {}
        }
    }

    pub fn setting_set_text(&mut self, idx: usize, val: String) {
        let v = if val.trim().is_empty() {
            None
        } else {
            Some(val.trim().to_string())
        };
        match idx {
            12 => {
                self.edit_cfg.voice_input_device = v.clone();
                self.voice_input_device = v;
            }
            13 => {
                self.edit_cfg.voice_output_device = v.clone();
                self.voice_output_device = v;
            }
            _ => {}
        }
    }

    pub fn save_config(&self) -> std::io::Result<()> {
        use serde_json::Value;
        let text = std::fs::read_to_string(&self.config_path)?;
        let mut root: Value =
            serde_json::from_str(&text).unwrap_or(Value::Object(Default::default()));
        if let Value::Object(map) = &mut root {
            let c = &self.edit_cfg;
            map.insert("download_media".into(), Value::Bool(c.download_media));
            map.insert("debug".into(), Value::Bool(c.debug));
            map.insert("print_muted_dm".into(), Value::Bool(c.print_muted_dm));
            map.insert("dm_track".into(), Value::Bool(c.dm_track));
            map.insert("server_track".into(), Value::Bool(c.server_track));
            map.insert("track_myself".into(), Value::Bool(c.track_myself));
            map.insert(
                "message_buffer_size".into(),
                Value::from(c.message_buffer_size),
            );
            map.insert(
                "voice_input_device".into(),
                c.voice_input_device
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            map.insert(
                "voice_output_device".into(),
                c.voice_output_device
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            let p = &self.prefs;
            map.insert(
                "ui".into(),
                serde_json::json!({
                    "toast_enabled": p.toast_enabled,
                    "toast_secs": p.toast_secs,
                    "notify_mentions": p.notify_mentions,
                    "show_relative_time": p.show_relative_time,
                    "show_typing": p.show_typing,
                    "dsp_rx": p.dsp_rx,
                    "dsp_tx": p.dsp_tx,
                    "self_mute": p.self_mute,
                    "self_deaf": p.self_deaf,
                }),
            );
        }
        let pretty = serde_json::to_string_pretty(&root)
            .unwrap_or_else(|_| "{}".to_string());
        std::fs::write(&self.config_path, pretty)
    }

    pub fn member_list(&self) -> (String, Vec<(String, PresenceStatus)>) {
        let rank = |s: PresenceStatus| match s {
            PresenceStatus::Online => 0,
            PresenceStatus::Idle => 1,
            PresenceStatus::Dnd => 2,
            PresenceStatus::Offline => 3,
        };

        let guild = self.current_guild.filter(|g| *g != DM_GUILD_ID);
        let mut ids: HashSet<SnowflakeID> = HashSet::new();
        let scope = if let Some(g) = guild {
            if let Some(roster) = self.guild_members.get(&g).filter(|r| !r.is_empty()) {
                ids.extend(roster.iter().copied());
            } else if let Some(chans) = self.channels_by_guild.get(&g) {
                for c in chans {
                    if let Some(ms) = self.messages.get(&c.id) {
                        for m in ms {
                            ids.insert(m.author_id);
                        }
                    }
                    if let Some(vm) = self.voice_members.get(&c.id) {
                        for v in vm {
                            ids.insert(v.id);
                        }
                    }
                }
            }
            self.guilds
                .iter()
                .find(|x| x.id == g)
                .map(|x| x.name.clone())
                .unwrap_or_else(|| "server".into())
        } else {
            ids.extend(self.user_dir.keys().copied());
            "everyone known".to_string()
        };
        if ids.is_empty() {
            ids.extend(self.user_dir.keys().copied());
        }

        let mut v: Vec<(String, PresenceStatus)> = ids
            .iter()
            .map(|id| (self.display_name(*id), self.presence_of(*id)))
            .collect();
        v.sort_by(|a, b| {
            rank(a.1)
                .cmp(&rank(b.1))
                .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
        });
        (scope, v)
    }

    pub fn mark_read(&mut self, channel_id: SnowflakeID) {
        self.unread.remove(&channel_id);
        self.unread_count.remove(&channel_id);
        if let Some(gid) = self.find_guild_for_channel(channel_id) {
            let any = self
                .channels_by_guild
                .get(&gid)
                .map(|cs| cs.iter().any(|c| self.unread.contains(&c.id)))
                .unwrap_or(false);
            if !any {
                self.unread.remove(&gid);
            }
        }
    }

    pub fn unread_n(&self, id: SnowflakeID) -> u32 {
        self.unread_count.get(&id).copied().unwrap_or(0)
    }

    pub fn find_guild_for_channel(&self, channel_id: SnowflakeID) -> Option<SnowflakeID> {
        self.channels_by_guild
            .iter()
            .find(|(_, chans)| chans.iter().any(|c| c.id == channel_id))
            .map(|(gid, _)| *gid)
    }

    pub fn clear_search(&mut self) {
        self.search_open = false;
        self.search_input.clear();
        self.search_cursor = 0;
        self.search_results.clear();
    }

    pub fn rebuild_search(&mut self) {
        let q = self.search_input.trim().to_lowercase();
        self.search_cursor = 0;
        self.search_results.clear();
        if q.is_empty() {
            return;
        }
        let guild_name: HashMap<SnowflakeID, String> =
            self.guilds.iter().map(|g| (g.id, g.name.clone())).collect();
        let score = |hay: &str| -> Option<i32> {
            let h = hay.to_lowercase();
            if h == q {
                Some(0)
            } else if h.starts_with(&q) {
                Some(1)
            } else if h.contains(&q) {
                Some(2)
            } else {
                None
            }
        };
        let mut hits: Vec<(i32, SearchHit)> = Vec::new();

        for g in &self.guilds {
            if g.id == DM_GUILD_ID {
                continue;
            }
            if let Some(s) = score(&g.name) {
                hits.push((
                    s,
                    SearchHit {
                        icon: '#',
                        label: g.name.clone(),
                        sub: "server".into(),
                        guild: Some(g.id),
                        channel: None,
                    },
                ));
            }
        }

        for (gid, chans) in &self.channels_by_guild {
            for c in chans {
                if let Some(s) = score(&c.name) {
                    let icon = match c.kind {
                        2 | 13 => '🔊',
                        1 | 3 => '@',
                        _ => '#',
                    };
                    let sub = if *gid == DM_GUILD_ID {
                        "direct message".to_string()
                    } else {
                        guild_name
                            .get(gid)
                            .cloned()
                            .unwrap_or_else(|| "server".into())
                    };
                    hits.push((
                        s + 1,
                        SearchHit {
                            icon,
                            label: c.name.clone(),
                            sub,
                            guild: Some(*gid),
                            channel: Some(c.id),
                        },
                    ));
                }
            }
        }

        let dm_for_user: HashMap<SnowflakeID, SnowflakeID> = self
            .channels_by_guild
            .get(&DM_GUILD_ID)
            .map(|v| {
                v.iter()
                    .filter_map(|c| c.recipient_id.map(|r| (r, c.id)))
                    .collect()
            })
            .unwrap_or_default();
        for (uid, name) in &self.user_dir {
            if let Some(s) = score(name) {
                let dm = dm_for_user.get(uid).copied();
                hits.push((
                    s + 1,
                    SearchHit {
                        icon: '@',
                        label: name.clone(),
                        sub: format!("user · {:?}", self.presence_of(*uid)).to_lowercase(),
                        guild: dm.map(|_| DM_GUILD_ID),
                        channel: dm,
                    },
                ));
            }
        }

        let mut msg_hits = 0;
        for (cid, msgs) in &self.messages {
            for m in msgs.iter().rev() {
                if msg_hits >= 40 {
                    break;
                }
                if m.content.to_lowercase().contains(&q) {
                    let owner = self.find_guild_for_channel(*cid);
                    let where_ = owner
                        .and_then(|g| guild_name.get(&g).cloned())
                        .unwrap_or_else(|| "direct message".into());
                    hits.push((
                        5,
                        SearchHit {
                            icon: '"',
                            label: format!("{}: {}", m.author, m.content),
                            sub: where_,
                            guild: owner,
                            channel: Some(*cid),
                        },
                    ));
                    msg_hits += 1;
                }
            }
        }

        hits.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.label.len().cmp(&b.1.label.len())));
        hits.truncate(80);
        self.search_results = hits.into_iter().map(|(_, h)| h).collect();
    }

    pub fn current_channels(&self) -> &[ChannelSummary] {
        self.current_guild
            .and_then(|gid| self.channels_by_guild.get(&gid))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn current_messages(&self) -> &[DisplayMessage] {
        self.current_channel
            .and_then(|cid| self.messages.get(&cid))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn push_message(&mut self, channel_id: SnowflakeID, msg: DisplayMessage) {
        let buf = self.messages.entry(channel_id).or_default();
        if buf.len() >= 500 {
            buf.remove(0);
        }
        buf.push(msg);
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_line = msg.into();
    }
}
