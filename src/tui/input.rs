use std::error::Error;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::rest;
use crate::tui::state::{AppState, Focus, SharedState};
use crate::utils::SnowflakeID;

pub async fn handle_event(
    event: Event,
    state: &SharedState,
    token: &str,
) -> Result<bool, Box<dyn Error>> {
    let Event::Key(key) = event else {
        return Ok(false);
    };
    if key.kind != KeyEventKind::Press {
        return Ok(false);
    }

    if key.code == KeyCode::Esc {
        let mut s = state.write().await;
        if s.search_open {
            s.clear_search();
            return Ok(true);
        }
        if s.settings_open {
            if s.settings_editing {
                s.settings_editing = false;
                s.settings_buf.clear();
            } else {
                s.settings_open = false;
                if let Err(e) = s.save_config() {
                    s.set_status(format!("config save failed: {e}"));
                } else {
                    s.set_status("settings saved".to_string());
                }
            }
            return Ok(true);
        }
        if s.show_help {
            s.show_help = false;
            return Ok(true);
        }
        if s.show_members {
            s.show_members = false;
            return Ok(true);
        }
        if s.editing_message.take().is_some() {
            s.input.clear();
            s.set_status("edit cancelled".to_string());
            return Ok(true);
        }
        if s.replying_to.take().is_some() {
            s.set_status("reply cancelled".to_string());
            return Ok(true);
        }
        if s.preview_image.take().is_some() {
            return Ok(true);
        }
        if !s.watching_streams.is_empty() {
            s.watching_streams.clear();
            s.big_streams.clear();
            let tx = s.gateway_tx.clone();
            s.set_status("stream view closed".to_string());
            if let Some(tx) = tx {
                let _ = tx.send(
                    crate::tui::ingest::GatewayAction::StopStream,
                );
            }
            return Ok(true);
        }
        s.should_quit = true;
        return Ok(true);
    }
    {
        let mut s = state.write().await;
        if s.settings_open {
            drop(s);
            return handle_settings_key(key, state).await;
        }
        if s.show_help {
            s.show_help = false;
            return Ok(true);
        }
        if s.show_members {
            s.show_members = false;
            return Ok(true);
        }
    }
    if key.code == KeyCode::Char('m') && state.read().await.focus != Focus::Input {
        let mut s = state.write().await;
        s.show_members = !s.show_members;
        if s.show_members {
            if let Some(g) = s
                .current_guild
                .filter(|g| *g != crate::tui::state::DM_GUILD_ID)
            {
                if s.members_requested.insert(g) {
                    if let Some(tx) = &s.gateway_tx {
                        let _ = tx.send(crate::tui::ingest::GatewayAction::RequestMembers {
                            guild: g,
                        });
                        s.set_status("requesting member list…".to_string());
                    }
                }
            }
        }
        return Ok(true);
    }
    if key.code == KeyCode::Char(',') && state.read().await.focus != Focus::Input {
        let mut s = state.write().await;
        s.settings_open = true;
        s.settings_cursor = 0;
        s.settings_editing = false;
        return Ok(true);
    }
    if key.code == KeyCode::Char('?') && state.read().await.focus != Focus::Input {
        let mut s = state.write().await;
        s.show_help = !s.show_help;
        return Ok(true);
    }
    if (key.code == KeyCode::Char('M') || key.code == KeyCode::Char('D'))
        && state.read().await.focus != Focus::Input
    {
        let deaf_key = key.code == KeyCode::Char('D');
        toggle_voice_state(state, deaf_key).await;
        return Ok(true);
    }
    if key.code == KeyCode::Char('b')
        && key.modifiers.contains(KeyModifiers::CONTROL)
    {
        let mut s = state.write().await;
        if !s.watching_streams.is_empty() {
            let all_big = s
                .watching_streams
                .iter()
                .all(|u| s.big_streams.contains(u));
            if all_big {
                s.big_streams.clear();
            } else {
                let ws = s.watching_streams.clone();
                for u in ws {
                    s.big_streams.insert(u);
                }
            }
            let m = if all_big { "PiP" } else { "grand (tous)" };
            s.set_status(format!("stream view: {m}"));
        }
        return Ok(true);
    }
    if key.modifiers.contains(KeyModifiers::ALT) && matches!(key.code, KeyCode::Down | KeyCode::Up)
    {
        let delta = if key.code == KeyCode::Down { 1 } else { -1 };
        jump_unread(state, delta).await;
        return Ok(true);
    }
    if key.code == KeyCode::Char('k') && key.modifiers.contains(KeyModifiers::CONTROL) {
        let mut s = state.write().await;
        if s.search_open {
            s.clear_search();
        } else {
            s.search_open = true;
            s.search_input.clear();
            s.search_cursor = 0;
            s.search_results.clear();
        }
        return Ok(true);
    }
    if state.read().await.search_open {
        return handle_search_key(key, state, token).await;
    }
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        state.write().await.should_quit = true;
        return Ok(true);
    }

    if key.code == KeyCode::Char('l') && key.modifiers.contains(KeyModifiers::CONTROL) {
        let mut s = state.write().await;
        s.show_logs = !s.show_logs;
        s.log_scroll = 0;
        return Ok(true);
    }

    let focus = state.read().await.focus;
    match focus {
        Focus::Input => handle_input_key(key, state, token).await,
        _ => handle_nav_key(key, state, token).await,
    }
}

async fn handle_nav_key(
    key: KeyEvent,
    state: &SharedState,
    token: &str,
) -> Result<bool, Box<dyn Error>> {
    let focus_now = state.read().await.focus;
    match key.code {
        KeyCode::Tab => {
            let mut s = state.write().await;
            s.focus = s.focus.next();
        }
        KeyCode::BackTab => {
            let mut s = state.write().await;
            s.focus = s.focus.prev();
        }
        KeyCode::Char('q') => {
            state.write().await.should_quit = true;
        }

        KeyCode::Left | KeyCode::Char('h') => {
            cursor_step_horizontal(state, -1).await;
        }
        KeyCode::Right | KeyCode::Char('l') => {
            cursor_step_horizontal(state, 1).await;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            cursor_step(state, 1).await;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            cursor_step(state, -1).await;
        }
        KeyCode::PageDown => {
            cursor_step(state, 10).await;
        }
        KeyCode::PageUp => {
            cursor_step(state, -10).await;
        }
        KeyCode::Enter => {
            open_selection(state, token).await?;
        }
        KeyCode::Char('d') | KeyCode::Delete if focus_now == Focus::Messages => {
            delete_selected(state, token).await?;
        }
        KeyCode::Char('i') if focus_now == Focus::Messages => {
            open_preview(state).await;
        }
        KeyCode::Char('e') if focus_now == Focus::Messages => {
            start_edit(state).await;
        }
        KeyCode::Char('r') if focus_now == Focus::Messages => {
            let mut s = state.write().await;
            let msgs = s.current_messages();
            let idx = s.message_cursor.min(msgs.len().saturating_sub(1));
            if let Some(m) = msgs.get(idx).cloned() {
                s.replying_to = Some(m.id);
                s.focus = Focus::Input;
                s.set_status(format!("replying to {}", m.author));
            }
        }
        KeyCode::Char('v') if focus_now == Focus::Channels => {
            join_selected_voice(state).await;
        }
        KeyCode::Char('V') => {
            leave_voice(state).await;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

const SLASH_COMMANDS: &[&str] = &[
    "/join ",
    "/leave",
    "/dm ",
    "/watch ",
    "/watch off",
    "/unwatch",
    "/big",
    "/stream",
    "/unstream",
    "/mute",
    "/deafen",
    "/call",
    "/upload ",
];

fn try_autocomplete(s: &mut AppState) -> bool {
    {
        let inp = s.input.clone();
        if inp.starts_with('/') && !inp.contains(' ') && inp.len() >= 2 {
            if let Some(c) = SLASH_COMMANDS
                .iter()
                .find(|c| c.starts_with(&inp) && **c != inp)
            {
                s.input = (*c).to_string();
                return true;
            }
        }
    }
    let input = s.input.clone();
    let start = input.rfind(char::is_whitespace).map(|i| i + 1).unwrap_or(0);
    let token = &input[start..];
    let sigil = token.as_bytes().first().copied().unwrap_or(0);
    if (sigil != b'@' && sigil != b'#') || token.len() < 2 {
        return false;
    }
    let prefix = token[1..].to_lowercase();
    let pick = |cands: Vec<String>| -> Option<String> {
        cands
            .iter()
            .find(|n| n.to_lowercase().starts_with(&prefix))
            .or_else(|| cands.iter().find(|n| n.to_lowercase().contains(&prefix)))
            .cloned()
    };
    let repl = match sigil {
        b'@' => {
            let names: Vec<String> = s.user_dir.values().cloned().collect();
            pick(names).map(|n| format!("@{n} "))
        }
        b'#' => {
            let guild = s.current_guild;
            let names: Vec<String> = guild
                .and_then(|g| s.channels_by_guild.get(&g))
                .map(|cs| {
                    cs.iter()
                        .filter(|c| c.is_textual())
                        .map(|c| c.name.clone())
                        .collect()
                })
                .unwrap_or_default();
            pick(names).map(|n| format!("#{n} "))
        }
        _ => None,
    };
    if let Some(r) = repl {
        s.input.truncate(start);
        s.input.push_str(&r);
        true
    } else {
        false
    }
}

async fn toggle_voice_state(state: &SharedState, deaf: bool) {
    use crate::tui::ingest::GatewayAction;
    use crate::voice::VoiceCommand;
    let (m, d, vtx, gtx) = {
        let mut s = state.write().await;
        if deaf {
            s.self_deaf = !s.self_deaf;
            if s.self_deaf {
                s.self_mute = true;
            }
        } else {
            s.self_mute = !s.self_mute;
            if !s.self_mute {
                s.self_deaf = false;
            }
        }
        let (m, d) = (s.self_mute, s.self_deaf);
        s.prefs.self_mute = m;
        s.prefs.self_deaf = d;
        let _ = s.save_config();
        let label = match (m, d) {
            (_, true) => "deafened (Shift+D / Shift+M)",
            (true, false) => "muted (Shift+M)",
            (false, false) => "live mic",
        };
        s.set_status(format!("voice: {label}"));
        (m, d, s.voice_cmd_tx.clone(), s.gateway_tx.clone())
    };
    if let Some(vtx) = vtx {
        let _ = vtx.send(VoiceCommand::SetMute(m));
        let _ = vtx.send(VoiceCommand::SetDeaf(d));
    }
    if let Some(gtx) = gtx {
        let _ = gtx.send(GatewayAction::SetVoiceState { mute: m, deaf: d });
    }
}

async fn cursor_step_horizontal(state: &SharedState, delta: isize) {
    let mut s = state.write().await;
    if s.focus != Focus::ServerBar || s.guilds.is_empty() {
        return;
    }
    s.guild_cursor = step(s.guild_cursor, delta, s.guilds.len());
}

async fn handle_input_key(
    key: KeyEvent,
    state: &SharedState,
    token: &str,
) -> Result<bool, Box<dyn Error>> {
    match key.code {
        KeyCode::Tab => {
            let mut s = state.write().await;
            if !try_autocomplete(&mut s) {
                s.focus = s.focus.next();
            }
        }
        KeyCode::BackTab => {
            let mut s = state.write().await;
            s.focus = s.focus.prev();
        }
        KeyCode::Char(c) => {
            let mut s = state.write().await;
            s.history_pos = None;
            s.input.push(c);
        }
        KeyCode::Backspace => {
            let mut s = state.write().await;
            s.history_pos = None;
            s.input.pop();
        }
        KeyCode::Up => {
            let mut s = state.write().await;
            let len = s.input_history.len();
            if len > 0 {
                let pos = match s.history_pos {
                    None => len - 1,
                    Some(0) => 0,
                    Some(p) => p - 1,
                };
                s.history_pos = Some(pos);
                s.input = s.input_history[pos].clone();
            }
        }
        KeyCode::Down => {
            let mut s = state.write().await;
            let len = s.input_history.len();
            match s.history_pos {
                Some(p) if p + 1 < len => {
                    s.history_pos = Some(p + 1);
                    s.input = s.input_history[p + 1].clone();
                }
                Some(_) => {
                    s.history_pos = None;
                    s.input.clear();
                }
                None => {}
            }
        }
        KeyCode::Enter => {
            send_input(state, token).await?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

async fn cursor_step(state: &SharedState, delta: isize) {
    let mut s = state.write().await;
    match s.focus {
        Focus::ServerBar => {
            if s.guilds.is_empty() {
                return;
            }
            s.guild_cursor = step(s.guild_cursor, delta, s.guilds.len());
        }
        Focus::Channels => {
            let len = s.current_channels().len();
            if len == 0 {
                return;
            }
            s.channel_cursor = step(s.channel_cursor, delta, len);
        }
        Focus::Messages => {
            let len = s.current_messages().len();
            if len == 0 {
                return;
            }

            s.message_cursor = step(s.message_cursor, -delta, len);
        }
        Focus::Input => {}
    }
}

fn step(cursor: usize, delta: isize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let max = len as isize - 1;
    let new = (cursor as isize + delta).clamp(0, max);
    new as usize
}

async fn open_selection(state: &SharedState, token: &str) -> Result<(), Box<dyn Error>> {
    let focus = state.read().await.focus;
    match focus {
        Focus::ServerBar => {
            let mut s = state.write().await;
            let Some(g) = s.guilds.get(s.guild_cursor).cloned() else {
                return Ok(());
            };
            s.current_guild = Some(g.id);
            s.channel_cursor = 0;
            s.current_channel = None;
            s.message_scroll = 0;
            s.focus = Focus::Channels;
            s.set_status(format!("guild → {}", g.name));
        }
        Focus::Channels => {
            let target = {
                let s = state.read().await;
                let chans = s.current_channels();
                let Some(c) = chans.get(s.channel_cursor).cloned() else {
                    return Ok(());
                };
                c
            };
            if !target.is_textual() {
                state
                    .write()
                    .await
                    .set_status(format!("{} is not a text channel", target.name));
                return Ok(());
            }
            {
                let mut s = state.write().await;
                s.current_channel = Some(target.id);
                s.message_scroll = 0;
                s.focus = Focus::Input;
                s.set_status(format!("loading #{}", target.name));

                s.mark_read(target.id);
            }
            backfill_channel(state, token, target.id).await?;
        }
        Focus::Messages => {
            state.write().await.focus = Focus::Input;
        }

        Focus::Input => {}
    }
    Ok(())
}

async fn handle_settings_key(
    key: KeyEvent,
    state: &SharedState,
) -> Result<bool, Box<dyn Error>> {
    use crate::tui::state::{SettingKind, SETTINGS};
    let mut s = state.write().await;
    if s.settings_editing {
        match key.code {
            KeyCode::Char(c) => s.settings_buf.push(c),
            KeyCode::Backspace => {
                s.settings_buf.pop();
            }
            KeyCode::Enter => {
                let idx = s.settings_cursor;
                let buf = std::mem::take(&mut s.settings_buf);
                s.setting_set_text(idx, buf);
                s.settings_editing = false;
            }
            _ => {}
        }
        return Ok(true);
    }
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            s.settings_cursor = (s.settings_cursor + 1).min(SETTINGS.len() - 1);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            s.settings_cursor = s.settings_cursor.saturating_sub(1);
        }
        KeyCode::Left | KeyCode::Char('h') => {
            let i = s.settings_cursor;
            s.setting_adjust(i, -1);
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Char(' ') => {
            let i = s.settings_cursor;
            s.setting_adjust(i, 1);
        }
        KeyCode::Enter => {
            let i = s.settings_cursor;
            if SETTINGS[i].kind == SettingKind::Text {
                s.settings_editing = true;
                s.settings_buf = s.setting_display(i);
                if s.settings_buf == "(default)" {
                    s.settings_buf.clear();
                }
            } else {
                s.setting_adjust(i, 1);
            }
        }
        _ => {}
    }
    Ok(true)
}

async fn handle_search_key(
    key: KeyEvent,
    state: &SharedState,
    token: &str,
) -> Result<bool, Box<dyn Error>> {
    match key.code {
        KeyCode::Down | KeyCode::Tab => {
            let mut s = state.write().await;
            let n = s.search_results.len();
            if n > 0 {
                s.search_cursor = (s.search_cursor + 1).min(n - 1);
            }
        }
        KeyCode::Up | KeyCode::BackTab => {
            let mut s = state.write().await;
            s.search_cursor = s.search_cursor.saturating_sub(1);
        }
        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let mut s = state.write().await;
            let n = s.search_results.len();
            if n > 0 {
                s.search_cursor = (s.search_cursor + 1).min(n - 1);
            }
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let mut s = state.write().await;
            s.search_cursor = s.search_cursor.saturating_sub(1);
        }
        KeyCode::Backspace => {
            let mut s = state.write().await;
            s.search_input.pop();
            s.rebuild_search();
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            let mut s = state.write().await;
            s.search_input.push(c);
            s.rebuild_search();
        }
        KeyCode::Enter => {
            let hit = {
                let s = state.read().await;
                s.search_results.get(s.search_cursor).cloned()
            };
            if let Some(hit) = hit {
                if let Some(ch) = hit.channel {
                    {
                        let mut s = state.write().await;
                        let guild = hit
                            .guild
                            .or_else(|| s.find_guild_for_channel(ch))
                            .unwrap_or(crate::tui::state::DM_GUILD_ID);
                        s.current_guild = Some(guild);
                        s.current_channel = Some(ch);
                        s.message_scroll = 0;
                        s.focus = Focus::Input;
                        s.mark_read(ch);
                        s.clear_search();
                    }
                    backfill_channel(state, token, ch).await?;
                } else if let Some(g) = hit.guild {
                    let mut s = state.write().await;
                    s.current_guild = Some(g);
                    s.current_channel = None;
                    s.channel_cursor = 0;
                    s.focus = Focus::Channels;
                    s.clear_search();
                } else {
                    state.write().await.clear_search();
                }
            } else {
                state.write().await.clear_search();
            }
        }
        _ => {}
    }
    Ok(true)
}

async fn backfill_channel(
    state: &SharedState,
    token: &str,
    channel_id: SnowflakeID,
) -> Result<(), Box<dyn Error>> {
    let msgs = match rest::fetch_messages(token, channel_id, 100).await {
        Ok(m) => m,
        Err(e) => {
            state
                .write()
                .await
                .set_status(format!("backfill failed: {e}"));
            return Ok(());
        }
    };
    let my_id = state.read().await.my_user_id;

    let mut display: Vec<crate::tui::state::DisplayMessage> = Vec::with_capacity(msgs.len());
    let mut image_jobs: Vec<(SnowflakeID, String)> = Vec::new();
    for m in msgs.into_iter().rev() {
        let (dm, imgs) = crate::tui::ingest::display_message_from_value(&m, my_id);
        image_jobs.extend(imgs);
        display.push(dm);
    }

    let (count, last_id) = {
        let mut s = state.write().await;
        let buf = s.messages.entry(channel_id).or_default();
        buf.clear();
        buf.append(&mut display);
        (buf.len(), buf.last().map(|m| m.id))
    };

    crate::tui::ingest::spawn_image_fetches(state, image_jobs);
    if let Some(mid) = last_id {
        let _ = rest::ack_message(token, channel_id, mid).await;
    }
    state
        .write()
        .await
        .set_status(format!("loaded {count} messages"));
    Ok(())
}

async fn delete_selected(state: &SharedState, token: &str) -> Result<(), Box<dyn Error>> {
    let target = {
        let s = state.read().await;
        let msgs = s.current_messages();
        let idx = s.message_cursor.min(msgs.len().saturating_sub(1));
        msgs.get(idx).cloned()
    };
    let Some(m) = target else {
        return Ok(());
    };
    if !m.is_self {
        state
            .write()
            .await
            .set_status("can only delete your own messages".to_string());
        return Ok(());
    }
    match rest::delete_message(token, m.channel_id, m.id).await {
        Ok(()) => {
            let mut s = state.write().await;
            if let Some(buf) = s.messages.get_mut(&m.channel_id) {
                buf.retain(|x| x.id != m.id);
            }
            s.set_status("deleted".to_string());
        }
        Err(e) => {
            state
                .write()
                .await
                .set_status(format!("delete failed: {e}"));
        }
    }
    Ok(())
}

async fn open_preview(state: &SharedState) {
    let target = {
        let s = state.read().await;
        let msgs = s.current_messages();
        let idx = s.message_cursor.min(msgs.len().saturating_sub(1));
        msgs.get(idx)
            .and_then(|m| m.attachment_images.first().map(|(id, _)| *id))
    };
    if let Some(id) = target {
        let mut s = state.write().await;
        if s.image_cache.contains_key(&id) {
            s.preview_image = Some(id);
        } else {
            s.set_status("image not loaded yet".to_string());
        }
    } else {
        state
            .write()
            .await
            .set_status("no image on this message".to_string());
    }
}

async fn join_selected_voice(state: &SharedState) {
    use crate::tui::ingest::GatewayAction;
    let (action, label) = {
        let s = state.read().await;
        let Some(guild) = s.current_guild else {
            return;
        };
        let chans = s.current_channels();
        let Some(c) = chans.get(s.channel_cursor) else {
            return;
        };

        if c.kind != 2 && c.kind != 13 {
            return;
        }
        (
            GatewayAction::JoinVoice {
                guild,
                channel: c.id,
            },
            c.name.clone(),
        )
    };
    let tx = state.read().await.gateway_tx.clone();
    if let Some(tx) = tx {
        let _ = tx.send(action);
        state
            .write()
            .await
            .set_status(format!("joining voice: {label}"));
    }
}

async fn leave_voice(state: &SharedState) {
    use crate::tui::ingest::GatewayAction;
    let (tx, guild) = {
        let s = state.read().await;
        (s.gateway_tx.clone(), s.joining_voice_guild)
    };
    if let (Some(tx), Some(guild)) = (tx, guild) {
        let _ = tx.send(GatewayAction::LeaveVoice { guild });
        state.write().await.set_status("leaving voice".to_string());
    }
}

async fn try_global_command(state: &SharedState, token: &str, raw: &str) -> bool {
    use crate::tui::ingest::GatewayAction;
    use crate::tui::state::{ChannelSummary, PendingStream, DM_GUILD_ID};
    let t = raw.trim();

    if let Some(code) = t.strip_prefix("/join ") {
        let code = code.trim().to_string();
        state.write().await.set_status(format!("joining {code}…"));
        match rest::accept_invite(token, &code).await {
            Ok(v) => {
                let name = v
                    .get("guild")
                    .and_then(|g| g.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("server")
                    .to_string();
                state
                    .write()
                    .await
                    .set_status(format!("joined {name}"));
            }
            Err(e) => state
                .write()
                .await
                .set_status(format!("join failed: {e}")),
        }
        return true;
    }

    if t == "/leave" {
        let guild = {
            let s = state.read().await;
            s.current_guild.filter(|g| *g != DM_GUILD_ID)
        };
        match guild {
            None => state
                .write()
                .await
                .set_status("not in a server (select a server channel)".to_string()),
            Some(g) => match rest::leave_guild(token, g).await {
                Ok(_) => {
                    let mut s = state.write().await;
                    s.channels_by_guild.remove(&g);
                    if s.current_guild == Some(g) {
                        s.current_guild = None;
                        s.current_channel = None;
                    }
                    s.set_status("left server".to_string());
                }
                Err(e) => state
                    .write()
                    .await
                    .set_status(format!("leave failed: {e}")),
            },
        }
        return true;
    }

    if let Some(arg) = t.strip_prefix("/dm ") {
        let arg = arg.trim();
        let Ok(uid) = arg.parse::<u64>() else {
            state
                .write()
                .await
                .set_status("usage: /dm <numeric user id>".to_string());
            return true;
        };
        let rid = SnowflakeID::from_u64(uid);
        match rest::create_dm(token, rid).await {
            Ok(v) => {
                let cid_u = v
                    .get("id")
                    .and_then(|x| x.as_str())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
                let cid = SnowflakeID::from_u64(cid_u);
                let name = v
                    .get("recipients")
                    .and_then(|r| r.as_array())
                    .and_then(|a| a.first())
                    .and_then(|u| {
                        u.get("global_name")
                            .or_else(|| u.get("username"))
                            .and_then(|n| n.as_str())
                    })
                    .unwrap_or("DM")
                    .to_string();
                let mut s = state.write().await;
                let dms = s.channels_by_guild.entry(DM_GUILD_ID).or_default();
                if !dms.iter().any(|c| c.id == cid) {
                    dms.push(ChannelSummary {
                        id: cid,
                        name: name.clone(),
                        recipient_id: Some(rid),
                        kind: 1,
                        parent_id: None,
                        position: 0,
                        last_activity: 0,
                        avatar_url: None,
                    });
                }
                s.current_guild = Some(DM_GUILD_ID);
                s.current_channel = Some(cid);
                s.focus = Focus::Messages;
                s.set_status(format!("DM with {name} opened"));
            }
            Err(e) => state
                .write()
                .await
                .set_status(format!("dm failed: {e}")),
        }
        return true;
    }

    if let Some(arg) = t.strip_prefix("/unwatch ") {
        if let Ok(uid) = arg.trim().parse::<u64>() {
            let sid = crate::utils::SnowflakeID::from_u64(uid);
            let tx = {
                let mut s = state.write().await;
                s.watching_streams.retain(|x| *x != sid);
                s.big_streams.remove(&sid);
                s.set_status(format!("stopped watching {uid}"));
                s.gateway_tx.clone()
            };
            if let Some(tx) = tx {
                let _ = tx.send(GatewayAction::StopOneStream { uid });
            }
        } else {
            state
                .write()
                .await
                .set_status("usage: /unwatch <uid> | /unwatch (all)".to_string());
        }
        return true;
    }
    if t == "/watch off" || t == "/unwatch" {
        let tx = {
            let mut s = state.write().await;
            s.watching_streams.clear();
            s.big_streams.clear();
            s.set_status("stream view closed".to_string());
            s.gateway_tx.clone()
        };
        if let Some(tx) = tx {
            let _ = tx.send(GatewayAction::StopStream);
        }
        return true;
    }
    if t == "/stream" {
        let tx = {
            let mut s = state.write().await;
            s.set_status("Go Live: requesting…".to_string());
            s.gateway_tx.clone()
        };
        if let Some(tx) = tx {
            let _ = tx.send(GatewayAction::GoLive);
        }
        return true;
    }
    if t == "/unstream" {
        let tx = {
            let mut s = state.write().await;
            s.set_status("Go Live: stopping".to_string());
            s.gateway_tx.clone()
        };
        if let Some(tx) = tx {
            let _ = tx.send(GatewayAction::StopGoLive);
        }
        return true;
    }
    if t == "/mute" {
        toggle_voice_state(state, false).await;
        return true;
    }
    if t == "/deafen" {
        toggle_voice_state(state, true).await;
        return true;
    }
    if let Some(arg) = t.strip_prefix("/big ") {
        if let Ok(uid) = arg.trim().parse::<u64>() {
            let sid = crate::utils::SnowflakeID::from_u64(uid);
            let mut s = state.write().await;
            let big = if s.big_streams.contains(&sid) {
                s.big_streams.remove(&sid);
                false
            } else {
                s.big_streams.insert(sid);
                true
            };
            s.set_status(format!(
                "stream {uid}: {}",
                if big { "grand" } else { "PiP" }
            ));
        } else {
            state
                .write()
                .await
                .set_status("usage: /big <uid> | /big (all)".to_string());
        }
        return true;
    }
    if t == "/big" {
        let mut s = state.write().await;
        if s.watching_streams.is_empty() {
            s.set_status("no stream to resize".to_string());
        } else {
            let all_big = s
                .watching_streams
                .iter()
                .all(|u| s.big_streams.contains(u));
            if all_big {
                s.big_streams.clear();
            } else {
                let ws = s.watching_streams.clone();
                for u in ws {
                    s.big_streams.insert(u);
                }
            }
            let m = if all_big { "PiP" } else { "grand (tous)" };
            s.set_status(format!("stream view: {m}"));
        }
        return true;
    }
    if let Some(arg) = t.strip_prefix("/watch ") {
        match arg.trim().parse::<u64>() {
            Ok(uid) => {
                let (key, tx) = {
                    let mut s = state.write().await;
                    let sid = crate::utils::SnowflakeID::from_u64(uid);
                    if !s.watching_streams.contains(&sid)
                        && s.watching_streams.len() < 4
                    {
                        s.watching_streams.push(sid);
                    }
                    let guild = s.current_guild;
                    let chan = s.voice_channel.or(s.current_channel);
                    let key = match (guild, chan) {
                        (Some(g), Some(c)) if g != DM_GUILD_ID => {
                            Some(format!("guild:{g}:{c}:{uid}"))
                        }
                        (_, Some(c)) => Some(format!("call:{c}:{uid}")),
                        _ => None,
                    };
                    if let Some(k) = &key {
                        s.pending_streams.insert(
                            k.clone(),
                            PendingStream {
                                uid,
                                ..Default::default()
                            },
                        );
                    }
                    let nstr = s.watching_streams.len();
                    s.set_status(format!(
                        "watching {uid} ({nstr} stream(s); Ctrl+B=resize, Esc=close)"
                    ));
                    (key, s.gateway_tx.clone())
                };
                if let (Some(key), Some(tx)) = (key, tx) {
                    let _ = tx.send(GatewayAction::WatchStream {
                        stream_key: key,
                        user: crate::utils::SnowflakeID::from_u64(uid),
                    });
                }
            }
            Err(_) => {
                state
                    .write()
                    .await
                    .set_status("usage: /watch <numeric user id> | /watch off".to_string());
            }
        }
        return true;
    }

    if t == "/call" {
        let (chan, guild, tx) = {
            let s = state.read().await;
            (
                s.current_channel,
                s.current_guild,
                s.gateway_tx.clone(),
            )
        };
        match (chan, guild, tx) {
            (Some(channel), Some(guild), Some(tx)) => {
                if guild == DM_GUILD_ID {
                    let _ = tx.send(GatewayAction::JoinDmCall { channel });
                } else {
                    let _ = tx.send(GatewayAction::JoinVoice { guild, channel });
                }
                state
                    .write()
                    .await
                    .set_status("calling…".to_string());
            }
            _ => state
                .write()
                .await
                .set_status("open a DM/channel first".to_string()),
        }
        return true;
    }

    false
}

async fn send_input(state: &SharedState, token: &str) -> Result<(), Box<dyn Error>> {
    {
        let raw = {
            let s = state.read().await;
            s.input.clone()
        };
        if raw.trim().is_empty() {
            return Ok(());
        }
        {
            let mut s = state.write().await;
            if s.input_history.last().map(|x| x.as_str())
                != Some(raw.as_str())
            {
                s.input_history.push(raw.clone());
                if s.input_history.len() > 200 {
                    s.input_history.remove(0);
                }
            }
            s.history_pos = None;
        }
        if raw.trim_start().starts_with('/')
            && (raw.starts_with("/join ")
                || raw.trim() == "/leave"
                || raw.starts_with("/dm ")
                || raw.starts_with("/watch ")
                || raw.trim() == "/unwatch"
                || raw.starts_with("/unwatch ")
                || raw.trim() == "/big"
                || raw.starts_with("/big ")
                || raw.trim() == "/stream"
                || raw.trim() == "/unstream"
                || raw.trim() == "/mute"
                || raw.trim() == "/deafen"
                || raw.trim() == "/call")
            && try_global_command(state, token, &raw).await
        {
            state.write().await.input.clear();
            return Ok(());
        }
    }
    let (channel_id, content, editing, reply_to) = {
        let mut s = state.write().await;
        if s.input.trim().is_empty() {
            return Ok(());
        }
        let Some(cid) = s.current_channel else {
            s.set_status("no channel selected".to_string());
            return Ok(());
        };
        let body = std::mem::take(&mut s.input);
        let editing = s.editing_message.take();
        let reply_to = s.replying_to.take();
        (cid, body, editing, reply_to)
    };

    if let Some(mid) = editing {
        match rest::edit_message(token, channel_id, mid, &content).await {
            Ok(_) => state.write().await.set_status("edited".to_string()),
            Err(e) => state.write().await.set_status(format!("edit failed: {e}")),
        }
        return Ok(());
    }

    let nonce = (chrono::Utc::now().timestamp_millis() as u64) << 22;
    if let Some(path) = content.strip_prefix("/upload ") {
        let path = path.trim().to_string();
        state
            .write()
            .await
            .set_status(format!("uploading {path}…"));
        match rest::upload_file(token, channel_id, &path, nonce).await {
            Ok(_) => state.write().await.set_status("uploaded".to_string()),
            Err(e) => state
                .write()
                .await
                .set_status(format!("upload failed: {e}")),
        }
        return Ok(());
    }
    match rest::send_message(token, channel_id, &content, nonce, reply_to).await {
        Ok(_) => state.write().await.set_status("sent".to_string()),
        Err(e) => state.write().await.set_status(format!("send failed: {e}")),
    }
    Ok(())
}

async fn start_edit(state: &SharedState) {
    let mut s = state.write().await;
    if s.focus != Focus::Messages {
        return;
    }
    let msgs = s.current_messages();
    let idx = s.message_cursor.min(msgs.len().saturating_sub(1));
    let Some(m) = msgs.get(idx).cloned() else {
        return;
    };
    if !m.is_self {
        s.set_status("can only edit your own messages".to_string());
        return;
    }
    s.editing_message = Some(m.id);
    s.input = m.content.clone();
    s.focus = Focus::Input;
    s.set_status("editing — Enter to save, Esc to cancel".to_string());
}

async fn jump_unread(state: &SharedState, delta: isize) {
    let mut s = state.write().await;
    let len = s.current_channels().len();
    if len == 0 {
        return;
    }
    let start = s.channel_cursor.min(len - 1);
    let mut i = start;
    for _ in 0..len {
        i = ((i as isize + delta).rem_euclid(len as isize)) as usize;
        let id = s.current_channels()[i].id;
        if s.unread.contains(&id) {
            s.channel_cursor = i;
            s.focus = Focus::Channels;
            return;
        }
    }
    s.set_status("no unread channels".to_string());
}
