use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};
use ratatui_image::{Resize, StatefulImage};
use std::collections::HashMap;

use crate::tui::Protocols;
use crate::tui::state::{AppState, Focus};
use crate::tui::stream_view::StreamView;
use crate::utils::SnowflakeID;

const SERVER_CELL_WIDTH: u16 = 14;
const SERVER_BAR_HEIGHT: u16 = 7;
const DM_GUILD_ID: SnowflakeID = SnowflakeID::const_zero();

pub fn draw(
    frame: &mut Frame,
    state: &AppState,
    protocols: &mut Protocols,
    streams: &mut HashMap<u64, StreamView>,
) {
    let area = frame.area();

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(SERVER_BAR_HEIGHT),
            Constraint::Min(5),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);

    let body = if state.show_logs {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(28),
                Constraint::Min(20),
                Constraint::Length(56),
            ])
            .split(outer[1])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(28), Constraint::Min(20)])
            .split(outer[1])
    };

    draw_server_bar(frame, state, outer[0], protocols);
    draw_left_panel(frame, state, body[0], protocols);
    draw_messages(frame, state, body[1], protocols);
    if state.show_logs {
        draw_logs(frame, state, body[2]);
    }
    draw_input(frame, state, outer[2]);
    draw_status(frame, state, outer[3]);

    draw_preview(frame, state, protocols);
    draw_stream(frame, state, streams);
    if state.search_open {
        draw_search(frame, state);
    }
    if state.settings_open {
        draw_settings(frame, state);
    }
    if state.show_help {
        draw_help(frame, state);
    }
    if state.show_members {
        draw_members(frame, state);
    }
    draw_toasts(frame, state);
}

fn draw_members(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    let modal = centered_rect(
        area.width.saturating_mul(5) / 10,
        area.height.saturating_mul(8) / 10,
        area,
    );
    frame.render_widget(Clear, modal);
    let (scope, people) = state.member_list();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(format!(
            "  Members · {scope} ({})  (m/Esc to close)  ",
            people.len()
        ));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let mut lines: Vec<Line> = Vec::new();
    for (name, st) in people.iter().take(inner.height as usize) {
        let col = match st {
            crate::tui::state::PresenceStatus::Online => Color::Green,
            crate::tui::state::PresenceStatus::Idle => Color::Yellow,
            crate::tui::state::PresenceStatus::Dnd => Color::Red,
            crate::tui::state::PresenceStatus::Offline => Color::DarkGray,
        };
        lines.push(Line::from(vec![
            Span::styled(format!(" {} ", st.glyph()), Style::default().fg(col)),
            Span::styled(
                truncate(name, inner.width.saturating_sub(4) as usize),
                Style::default().fg(Color::Gray),
            ),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

const HELP_LINES: &[(&str, &str)] = &[
    (
        "Tab / Shift+Tab",
        "cycle focus (servers ▸ channels ▸ messages ▸ input)",
    ),
    ("← → ↑ ↓ / h j k l", "move within the focused pane"),
    ("PageUp / PageDown", "scroll messages by 10"),
    ("Enter", "open server / channel / send message"),
    (
        "Ctrl+K",
        "global search (people, channels, servers, messages)",
    ),
    ("? ", "this help"),
    ("m", "members list (with presence)"),
    (", ", "settings panel"),
    ("/upload <path>", "send a file (type in the message box)"),
    ("↑ / ↓ (in input)", "browse sent message/command history"),
    ("Tab (in input)", "autocomplete /command, @user, #channel"),
    ("Ctrl+L", "toggle log panel"),
    ("Alt+↓ / Alt+↑", "jump to next / previous unread"),
    ("e", "edit selected message (your own)"),
    ("r", "reply to selected message"),
    ("d / Del", "delete selected message"),
    ("i", "preview selected image fullscreen"),
    ("v / V", "join / leave voice (on a voice channel)"),
    ("Shift+M / Shift+D", "toggle mic mute / deafen (in voice)"),
    (
        "/watch <user id>",
        "watch someone's screen share (≤4, repeat)",
    ),
    (
        "/big <uid> · Ctrl+B",
        "resize one stream big/PiP · Ctrl+B = all",
    ),
    (
        "/unwatch <uid>",
        "stop one stream · /watch off or Esc = all",
    ),
    (
        "/join /leave /dm /call",
        "invite · leave server · Create new DM · DM call",
    ),
    ("q / Ctrl+C", "quit"),
    ("Esc", "close overlay / cancel edit / quit"),
];

fn draw_help(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    let modal = centered_rect(
        area.width.saturating_mul(7) / 10,
        area.height.saturating_mul(8) / 10,
        area,
    );
    frame.render_widget(Clear, modal);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title("  Keybindings & features  (? or Esc to close)  ");
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let _ = state;
    let mut lines: Vec<Line> = Vec::new();
    for (k, v) in HELP_LINES {
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {:<18}", k),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(v.to_string(), Style::default().fg(Color::Gray)),
        ]));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn draw_settings(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    let modal = centered_rect(
        area.width.saturating_mul(6) / 10,
        area.height.saturating_mul(8) / 10,
        area,
    );
    frame.render_widget(Clear, modal);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Green))
        .title("  Settings  (↑↓ move · ←→/Space change · Enter edit text · Esc save & close)  ");
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    if inner.height == 0 {
        return;
    }
    let visible = inner.height as usize;
    let total = crate::tui::state::SETTINGS.len();
    let cursor = state.settings_cursor.min(total.saturating_sub(1));
    let first = cursor
        .saturating_sub(visible.saturating_sub(1))
        .min(total.saturating_sub(visible.min(total)));
    let mut lines: Vec<Line> = Vec::new();
    for (i, row) in crate::tui::state::SETTINGS
        .iter()
        .enumerate()
        .skip(first)
        .take(visible)
    {
        let selected = i == cursor;
        let editing = selected && state.settings_editing;
        let val = if editing {
            format!("{}▏", state.settings_buf)
        } else {
            state.setting_display(i)
        };
        let base = if selected {
            Style::default()
                .bg(Color::Green)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let val_style = if editing {
            Style::default().fg(Color::Black).bg(Color::Yellow)
        } else {
            base
        };
        lines.push(Line::from(vec![
            Span::styled(format!(" {:<32}", row.label), base),
            Span::styled(format!(" {val}"), val_style),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_toasts(frame: &mut Frame, state: &AppState) {
    let toasts = state.active_toasts();
    if toasts.is_empty() {
        return;
    }
    let area = frame.area();
    let w: u16 = 46.min(area.width.saturating_sub(2));
    let h: u16 = 5;
    let mut y = area.y + SERVER_BAR_HEIGHT + 1;
    let accent = Color::Rgb(88, 101, 242);
    for t in toasts {
        if y + h > area.y + area.height {
            break;
        }
        let rect = Rect {
            x: area.x + area.width.saturating_sub(w + 2),
            y,
            width: w,
            height: h,
        };
        frame.render_widget(Clear, rect);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(accent))
            .style(Style::default().bg(Color::Rgb(20, 21, 28)))
            .title(Span::styled(
                format!(" ✦ {} ", truncate(&t.title, (w as usize).saturating_sub(6))),
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ));
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        let bar = Rect {
            x: inner.x,
            y: inner.y,
            width: 1,
            height: inner.height,
        };
        let text_area = Rect {
            x: inner.x + 2,
            y: inner.y,
            width: inner.width.saturating_sub(2),
            height: inner.height,
        };
        frame.render_widget(Block::default().style(Style::default().bg(accent)), bar);
        let body = Paragraph::new(truncate(
            &t.body,
            (text_area.width as usize) * (text_area.height as usize),
        ))
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::White).bg(Color::Rgb(20, 21, 28)));
        frame.render_widget(body, text_area);
        y += h + 1;
    }
}

fn centered_rect(w: u16, h: u16, area: Rect) -> Rect {
    let cw = w.min(area.width);
    let ch = h.min(area.height);
    Rect {
        x: area.x + (area.width - cw) / 2,
        y: area.y + (area.height - ch) / 2,
        width: cw,
        height: ch,
    }
}

fn draw_search(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    let modal = centered_rect(
        area.width.saturating_mul(7) / 10,
        area.height.saturating_mul(8) / 10,
        area,
    );
    frame.render_widget(Clear, modal);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Magenta))
        .title("  Search  (type to filter · ↑↓ select · Enter open · Esc close)  ");
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    if inner.height < 2 {
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    let query = Paragraph::new(Line::from(vec![
        Span::styled(
            "❯ ",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            state.search_input.clone(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("▏", Style::default().fg(Color::Magenta)),
    ]));
    frame.render_widget(query, rows[0]);

    let list_area = rows[1];
    let visible = list_area.height as usize;
    let total = state.search_results.len();
    if total == 0 {
        let msg = if state.search_input.trim().is_empty() {
            "Type to search people, channels, servers and messages…"
        } else {
            "No matches."
        };
        frame.render_widget(
            Paragraph::new(Span::styled(msg, Style::default().fg(Color::DarkGray))),
            list_area,
        );
        return;
    }
    let cursor = state.search_cursor.min(total - 1);
    let first = cursor
        .saturating_sub(visible.saturating_sub(1))
        .min(total.saturating_sub(visible.min(total)));
    let width = list_area.width as usize;
    let mut lines: Vec<Line> = Vec::new();
    for (i, hit) in state
        .search_results
        .iter()
        .enumerate()
        .skip(first)
        .take(visible)
    {
        let selected = i == cursor;
        let label = truncate(&hit.label, width.saturating_sub(hit.sub.len() + 6));
        let base = if selected {
            Style::default()
                .bg(Color::Magenta)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", hit.icon), base),
            Span::styled(label, base),
            Span::styled(
                format!("  — {}", hit.sub),
                if selected {
                    base
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), list_area);
}

fn draw_logs(frame: &mut Frame, state: &AppState, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" logs — Ctrl+L ");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let rows = inner.height as usize;
    let lines = crate::utils::recent_logs(rows + state.log_scroll);
    let end = lines.len().saturating_sub(state.log_scroll);
    let start = end.saturating_sub(rows);
    let slice = &lines[start..end];

    let rendered: Vec<Line> = slice
        .iter()
        .map(|l| {
            let color = if l.contains("[Erreur]") {
                Color::Red
            } else if l.contains("[Avertissement]") {
                Color::Yellow
            } else if l.contains("[Information]") || l.contains("[Login]") {
                Color::Green
            } else if l.contains("[Voice") {
                Color::Cyan
            } else {
                Color::Gray
            };
            Line::from(Span::styled(l.clone(), Style::default().fg(color)))
        })
        .collect();

    let p = Paragraph::new(rendered).wrap(Wrap { trim: false });
    frame.render_widget(p, inner);
}

fn focused_style(state: &AppState, target: Focus) -> Style {
    if state.focus == target {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn draw_server_bar(frame: &mut Frame, state: &AppState, area: Rect, protocols: &mut Protocols) {
    let title = if state.image_backend.is_empty() {
        "Servers < >".to_string()
    } else {
        format!("Servers < >  [img: {}]", state.image_backend)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(focused_style(state, Focus::ServerBar))
        .title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.guilds.is_empty() || inner.width == 0 {
        return;
    }

    let visible_cells = (inner.width / SERVER_CELL_WIDTH).max(1) as usize;
    let cursor = state.guild_cursor.min(state.guilds.len().saturating_sub(5));
    let first = cursor.saturating_sub(visible_cells / 2);
    let first = first.min(
        state
            .guilds
            .len()
            .saturating_sub(visible_cells.min(state.guilds.len())),
    );
    let last = (first + visible_cells).min(state.guilds.len());

    let constraints: Vec<Constraint> = (first..last)
        .map(|_| Constraint::Length(SERVER_CELL_WIDTH))
        .collect();
    let cells = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(inner);

    for (idx, slot) in (first..last).enumerate() {
        let g = &state.guilds[slot];
        let selected = slot == cursor;
        let cell = cells[idx];
        if cell.height < 2 {
            continue;
        }

        let icon_h = (cell.height - 1).min(4);
        let icon_w = (icon_h * 2).min(cell.width);
        let icon_area = Rect {
            x: cell.x + (cell.width - icon_w) / 2,
            y: cell.y,
            width: icon_w,
            height: icon_h,
        };
        let name_area = Rect {
            x: cell.x,
            y: cell.y + cell.height - 1,
            width: cell.width,
            height: 1,
        };

        let mut drew_image = false;
        if g.id != DM_GUILD_ID {
            if let Some(proto) = protocols.get_mut(&g.id) {
                let img = StatefulImage::default().resize(Resize::Fit(None));
                frame.render_stateful_widget(img, icon_area, proto);
                drew_image = true;
            }
        }
        if !drew_image {
            let label = if g.id == DM_GUILD_ID {
                "@".to_string()
            } else {
                g.name
                    .chars()
                    .find(|c| c.is_alphanumeric())
                    .map(|c| c.to_uppercase().to_string())
                    .unwrap_or_else(|| "#".to_string())
            };
            let style = if g.id == DM_GUILD_ID {
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Indexed(((u64::from(g.id) % 6) as u8) + 1))
                    .add_modifier(Modifier::BOLD)
            };
            let mut lines = Vec::with_capacity(icon_area.height as usize);
            for _ in 0..(icon_area.height.saturating_sub(1) / 2) {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled(format!(" {label} "), style)));
            let p = Paragraph::new(lines).alignment(ratatui::layout::Alignment::Center);
            frame.render_widget(p, icon_area);
        }

        let unread = state.unread.contains(&g.id);
        let raw = if g.id == DM_GUILD_ID {
            "Direct".to_string()
        } else {
            g.name.replace(|c: char| c.is_control(), "")
        };
        let dot = if unread { "●" } else { "" };
        let name = truncate(
            &raw,
            cell.width.saturating_sub(1 + dot.len() as u16) as usize,
        );
        let name_style = if selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else if unread {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let label = if unread { format!("●{name}") } else { name };
        let p = Paragraph::new(Line::from(Span::styled(label, name_style)))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(p, name_area);
    }
}

fn draw_left_panel(frame: &mut Frame, state: &AppState, area: Rect, protocols: &mut Protocols) {
    let in_dms = state.current_guild == Some(DM_GUILD_ID);
    if in_dms {
        draw_dm_list(frame, state, area, protocols);
    } else {
        draw_channel_list(frame, state, area);
    }
}

fn draw_dm_list(frame: &mut Frame, state: &AppState, area: Rect, protocols: &mut Protocols) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(focused_style(state, Focus::Channels))
        .title("Direct Messages");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let channels = state.current_channels();
    if channels.is_empty() || inner.height == 0 {
        return;
    }

    const ROW: u16 = 2;
    let visible = (inner.height / ROW).max(1) as usize;
    let cursor = state.channel_cursor.min(channels.len().saturating_sub(1));
    let first = cursor
        .saturating_sub(visible.saturating_sub(1))
        .min(channels.len().saturating_sub(visible.min(channels.len())));
    let last = (first + visible).min(channels.len());

    for (i, slot) in (first..last).enumerate() {
        let c = &channels[slot];
        let y = inner.y + (i as u16) * ROW;
        if y + 1 >= inner.y + inner.height {
            break;
        }

        let selected = slot == cursor;
        let avatar_rect = Rect {
            x: inner.x,
            y,
            width: 4,
            height: 2,
        };
        let name_rect = Rect {
            x: inner.x + 5,
            y: y + 1,
            width: inner.width.saturating_sub(5),
            height: 1,
        };

        if let Some(proto) = protocols.get_mut(&c.id) {
            let img = StatefulImage::default();
            frame.render_stateful_widget(img, avatar_rect, proto);
        } else {
            let placeholder = if c.kind == 3 { "👥" } else { "@" };
            let p = Paragraph::new(placeholder).alignment(ratatui::layout::Alignment::Center);
            frame.render_widget(p, avatar_rect);
        }

        let unread = state.unread.contains(&c.id);
        let style = if selected {
            Style::default()
                .bg(Color::Magenta)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else if unread {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };

        let (dot, dot_color) = match c.recipient_id {
            Some(uid) => {
                let st = state.presence_of(uid);
                let col = match st {
                    crate::tui::state::PresenceStatus::Online => Color::Green,
                    crate::tui::state::PresenceStatus::Idle => Color::Yellow,
                    crate::tui::state::PresenceStatus::Dnd => Color::Red,
                    crate::tui::state::PresenceStatus::Offline => Color::DarkGray,
                };
                (st.glyph(), col)
            }
            None => (" ", Color::DarkGray),
        };

        let n = state.unread_n(c.id);
        let unread_mark = if n > 0 {
            format!("●{n} ")
        } else if unread {
            "● ".to_string()
        } else {
            "  ".to_string()
        };

        let typers = if state.prefs.show_typing {
            state.typing_names(c.id)
        } else {
            Vec::new()
        };
        let body = if !typers.is_empty() {
            let who = if typers.len() == 1 {
                format!("{} is typing…", typers[0])
            } else {
                format!("{} people typing…", typers.len())
            };
            Span::styled(
                truncate(&who, (name_rect.width as usize).saturating_sub(3)),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::ITALIC),
            )
        } else {
            Span::styled(
                truncate(&c.name, (name_rect.width as usize).saturating_sub(3)),
                style,
            )
        };
        let line = Line::from(vec![
            Span::styled(dot, Style::default().fg(dot_color)),
            Span::raw(unread_mark),
            Span::raw(" "),
            body,
        ]);
        frame.render_widget(Paragraph::new(line), name_rect);
    }
}

fn draw_channel_list(frame: &mut Frame, state: &AppState, area: Rect) {
    let channels = state.current_channels();
    let items: Vec<ListItem> = channels
        .iter()
        .map(|c| {
            let is_thread = c.name.starts_with('↳');
            let prefix = match c.kind {
                _ if is_thread => "      ",
                0 => "  # ",
                5 => "  📢 ",
                2 => "  🔊 ",
                13 => "  🎤 ",
                4 => "",
                11 | 12 => "      ↪ ",
                15 => "  📂 ",
                _ => "  ",
            };
            let unread = state.unread.contains(&c.id);
            let style = if c.kind == 4 {
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else if unread {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let n = state.unread_n(c.id);
            let marker = if n > 0 {
                format!("●{n} ")
            } else if unread {
                "● ".to_string()
            } else {
                String::new()
            };
            let text = if c.kind == 4 {
                format!("▾ {}", truncate(&c.name.to_uppercase(), 20))
            } else {
                format!("{marker}{prefix}{}", truncate(&c.name, 20))
            };
            let mut lines = vec![Line::from(Span::styled(text, style))];

            if c.kind == 2 || c.kind == 13 {
                if let Some(members) = state.voice_members.get(&c.id) {
                    for vm in members {
                        let disp = state
                            .user_dir
                            .get(&vm.id)
                            .cloned()
                            .unwrap_or_else(|| vm.name.clone());
                        let mut flags = String::new();
                        if vm.deaf {
                            flags.push_str(" 🔇");
                        } else if vm.mute {
                            flags.push_str(" 🔈");
                        }
                        if vm.video {
                            flags.push_str(" 📹");
                        }
                        if vm.stream {
                            flags.push_str(" 🔴");
                        }
                        lines.push(Line::from(Span::styled(
                            format!("    ◦ {}{}", truncate(&disp, 16), flags),
                            Style::default().fg(Color::Green),
                        )));
                    }
                }
            }
            ListItem::new(lines)
        })
        .collect();

    let mut list_state = ListState::default();
    if !channels.is_empty() {
        list_state.select(Some(state.channel_cursor.min(channels.len() - 1)));
    }

    let title = state
        .current_guild
        .and_then(|gid| state.guilds.iter().find(|g| g.id == gid))
        .map(|g| truncate(&g.name, 24).to_string())
        .unwrap_or_else(|| "Channels".to_string());

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(focused_style(state, Focus::Channels))
                .title(title),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut list_state);
}

const THUMB_HEIGHT: u16 = 4;
const THUMB_WIDTH: u16 = 12;

fn message_height(m: &crate::tui::state::DisplayMessage) -> u16 {
    1 + (m.attachment_images.len() as u16) * THUMB_HEIGHT
        + m.reply_to.is_some() as u16
        + (!m.reactions.is_empty()) as u16
}

fn resolve_mentions(content: &str, state: &AppState) -> String {
    let mut out = String::with_capacity(content.len());
    let bytes = content.as_bytes();
    let mut i = 0;
    while i < content.len() {
        if bytes[i] == b'<' {
            if let Some(end) = content[i..].find('>') {
                let tok = &content[i + 1..i + end];
                let resolved = if let Some(rest) = tok
                    .strip_prefix("@!")
                    .or_else(|| tok.strip_prefix('@'))
                    .filter(|r| r.chars().all(|c| c.is_ascii_digit()))
                {
                    rest.parse::<u64>()
                        .ok()
                        .map(|n| format!("@{}", state.display_name(SnowflakeID::from_u64(n))))
                } else if let Some(rest) = tok.strip_prefix('#') {
                    rest.parse::<u64>().ok().map(|n| {
                        let cid = SnowflakeID::from_u64(n);
                        let nm = state
                            .channels_by_guild
                            .values()
                            .flatten()
                            .find(|c| c.id == cid)
                            .map(|c| c.name.clone())
                            .unwrap_or_else(|| "channel".into());
                        format!("#{nm}")
                    })
                } else if let Some(rest) = tok.strip_prefix("@&") {
                    let _ = rest;
                    Some("@role".to_string())
                } else if tok.starts_with(':') || tok.starts_with("a:") {
                    tok.trim_start_matches("a:")
                        .trim_matches(':')
                        .split(':')
                        .next()
                        .map(|n| format!(":{n}:"))
                } else {
                    None
                };
                if let Some(r) = resolved {
                    out.push_str(&r);
                    i += end + 1;
                    continue;
                }
            }
        }
        let ch = content[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn markdown_spans(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut buf = String::new();
    let mut chars = text.chars().peekable();
    let flush = |spans: &mut Vec<Span<'static>>, buf: &mut String| {
        if !buf.is_empty() {
            spans.push(Span::raw(std::mem::take(buf)));
        }
    };
    while let Some(c) = chars.next() {
        match c {
            '`' => {
                let mut code = String::new();
                while let Some(&n) = chars.peek() {
                    chars.next();
                    if n == '`' {
                        break;
                    }
                    code.push(n);
                }
                flush(&mut spans, &mut buf);
                spans.push(Span::styled(
                    code,
                    Style::default()
                        .fg(Color::Rgb(220, 220, 170))
                        .bg(Color::Rgb(40, 40, 40)),
                ));
            }
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut bold = String::new();
                while let Some(&n) = chars.peek() {
                    chars.next();
                    if n == '*' && chars.peek() == Some(&'*') {
                        chars.next();
                        break;
                    }
                    bold.push(n);
                }
                flush(&mut spans, &mut buf);
                spans.push(Span::styled(
                    bold,
                    Style::default().add_modifier(Modifier::BOLD),
                ));
            }
            '*' | '_' => {
                let close = c;
                let mut it = String::new();
                while let Some(&n) = chars.peek() {
                    chars.next();
                    if n == close {
                        break;
                    }
                    it.push(n);
                }
                flush(&mut spans, &mut buf);
                spans.push(Span::styled(
                    it,
                    Style::default().add_modifier(Modifier::ITALIC),
                ));
            }
            _ => buf.push(c),
        }
    }
    flush(&mut spans, &mut buf);
    spans
}

fn draw_messages(frame: &mut Frame, state: &AppState, area: Rect, protocols: &mut Protocols) {
    let title = state
        .current_channel
        .and_then(|cid| {
            state
                .current_channels()
                .iter()
                .find(|c| c.id == cid)
                .map(|c| {
                    if let Some(uid) = c.recipient_id {
                        let st = state.presence_of(uid);
                        format!("  {} {} ({:?})  ", st.glyph(), c.name, st)
                    } else {
                        format!("  {}  ", c.name)
                    }
                })
        })
        .unwrap_or_else(|| "Messages".to_string());

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(focused_style(state, Focus::Messages))
        .title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let msgs = state.current_messages();
    if msgs.is_empty() || inner.height == 0 {
        return;
    }
    let cursor = state.message_cursor.min(msgs.len().saturating_sub(1));
    let in_messages = state.focus == Focus::Messages;

    let anchor = if in_messages { cursor } else { msgs.len() - 1 };
    let mut visible: Vec<usize> = Vec::new();
    let mut used = 0u16;
    for idx in (0..=anchor).rev() {
        let h = message_height(&msgs[idx]);
        if used + h > inner.height {
            break;
        }
        used += h;
        visible.push(idx);
    }
    visible.reverse();

    for idx in (anchor + 1)..msgs.len() {
        let h = message_height(&msgs[idx]);
        if used + h > inner.height {
            break;
        }
        used += h;
        visible.push(idx);
    }

    let mut y = inner.y;
    for idx in visible {
        let m = &msgs[idx];
        let h = message_height(m);
        let row_rect = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: h,
        };

        let selected = in_messages && idx == cursor;
        if selected {
            frame.render_widget(
                Block::default().style(Style::default().bg(Color::Rgb(40, 40, 60))),
                row_rect,
            );
        }

        let mut line_y = y;
        if let Some((who, snip)) = &m.reply_to {
            let rr = Rect {
                x: inner.x,
                y: line_y,
                width: inner.width,
                height: 1,
            };
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("  ↪ ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{who}: "), Style::default().fg(Color::Blue)),
                    Span::styled(
                        truncate(snip, inner.width.saturating_sub(8) as usize),
                        Style::default().fg(Color::DarkGray),
                    ),
                ])),
                rr,
            );
            line_y += 1;
        }
        let text_rect = Rect {
            x: inner.x,
            y: line_y,
            width: inner.width,
            height: 1,
        };

        let ts = if state.prefs.show_relative_time {
            format!("{} {}", m.timestamp.format("%H:%M"), rel_time(m.timestamp))
        } else {
            m.timestamp.format("%H:%M").to_string()
        };
        let mentions_me = state
            .my_user_id
            .map(|id| {
                let needle = format!("<@{}", u64::from(id));
                m.content.contains(&needle)
            })
            .unwrap_or(false)
            || m.content.contains("@everyone")
            || m.content.contains("@here");
        if mentions_me && !selected {
            frame.render_widget(
                Block::default().style(Style::default().bg(Color::Rgb(60, 50, 20))),
                row_rect,
            );
        }
        let author_style = if m.is_self {
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        };
        let mut spans = vec![Span::styled(
            format!("{ts} "),
            Style::default().fg(Color::DarkGray),
        )];
        if mentions_me {
            spans.push(Span::styled(
                "▎",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        spans.push(Span::styled(m.author.clone(), author_style));
        spans.push(Span::raw(": "));
        let resolved = resolve_mentions(&m.content, state);
        spans.extend(markdown_spans(&resolved));
        if m.edited {
            spans.push(Span::styled(
                " (edited)",
                Style::default().fg(Color::DarkGray),
            ));
        }
        if !m.attachment_images.is_empty() {
            spans.push(Span::styled(
                format!("  [{}🖼]", m.attachment_images.len()),
                Style::default().fg(Color::Yellow),
            ));
        }
        let p = Paragraph::new(Line::from(spans)).wrap(Wrap { trim: false });
        frame.render_widget(p, text_rect);

        let mut below = line_y + 1;
        if !m.reactions.is_empty() {
            let mut rspans = vec![Span::raw("   ")];
            for (name, count) in &m.reactions {
                rspans.push(Span::styled(
                    format!(" {name} {count} "),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Rgb(80, 80, 110)),
                ));
                rspans.push(Span::raw(" "));
            }
            frame.render_widget(
                Paragraph::new(Line::from(rspans)),
                Rect {
                    x: inner.x,
                    y: below,
                    width: inner.width,
                    height: 1,
                },
            );
            below += 1;
        }

        for (i, (img_id, _)) in m.attachment_images.iter().enumerate() {
            let thumb_y = below + (i as u16) * THUMB_HEIGHT;
            if thumb_y + THUMB_HEIGHT > inner.y + inner.height {
                break;
            }
            let thumb_rect = Rect {
                x: inner.x + 8,
                y: thumb_y,
                width: THUMB_WIDTH.min(inner.width.saturating_sub(8)),
                height: THUMB_HEIGHT,
            };
            if let Some(proto) = protocols.get_mut(img_id) {
                let img = StatefulImage::default().resize(Resize::Fit(None));
                frame.render_stateful_widget(img, thumb_rect, proto);
            } else {
                let p = Paragraph::new(Span::styled(
                    "  (loading…)",
                    Style::default().fg(Color::DarkGray),
                ));
                frame.render_widget(p, thumb_rect);
            }
        }

        y += h;
        if y >= inner.y + inner.height {
            break;
        }
    }
}

fn draw_preview(frame: &mut Frame, state: &AppState, protocols: &mut Protocols) {
    let Some(id) = state.preview_image else {
        return;
    };
    let area = frame.area();

    frame.render_widget(
        Block::default().style(Style::default().bg(Color::Black)),
        area,
    );

    let img_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: area.height.saturating_sub(1),
    };
    let hint_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(1),
        width: area.width,
        height: 1,
    };

    if let Some(proto) = protocols.get_mut(&id) {
        let img = StatefulImage::default().resize(Resize::Fit(None));
        frame.render_stateful_widget(img, img_area, proto);
    } else {
        let p = Paragraph::new("image not in cache (anymore?)")
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(p, img_area);
    }

    let hint = Paragraph::new(" image preview — Esc to close ")
        .style(Style::default().fg(Color::Black).bg(Color::Yellow));
    frame.render_widget(hint, hint_area);
}

fn draw_stream(frame: &mut Frame, state: &AppState, streams: &mut HashMap<u64, StreamView>) {
    let uids = state.watching_streams.clone();
    if uids.is_empty() {
        return;
    }
    let scr = frame.area();
    let stat_y = scr.y + SERVER_BAR_HEIGHT;
    let top = stat_y + 1;
    let region_h = scr.height.saturating_sub(SERVER_BAR_HEIGHT + 2);
    {
        let st = crate::voice::video::stats();
        let pending = st.recv.saturating_sub(st.decoded + st.dropped);
        let line = format!(
            " ⬤ stream  fps:{:.0}  buf:{}  recv:{}  decoded:{}  dropped:{}  pending:{} ",
            st.fps, st.qlen, st.recv, st.decoded, st.dropped, pending
        );
        let bar = Rect {
            x: scr.x,
            y: stat_y,
            width: scr.width,
            height: 1,
        };
        frame.render_widget(Clear, bar);
        frame.render_widget(
            Paragraph::new(line).style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(88, 101, 242))
                    .add_modifier(Modifier::BOLD),
            ),
            bar,
        );
    }
    let pip: Vec<SnowflakeID> = uids
        .iter()
        .filter(|u| !state.big_streams.contains(u))
        .cloned()
        .collect();
    let big: Vec<SnowflakeID> = uids
        .iter()
        .filter(|u| state.big_streams.contains(u))
        .cloned()
        .collect();
    let mut rect_of: HashMap<u64, Rect> = HashMap::new();
    let pip_w = if pip.is_empty() {
        0
    } else {
        (scr.width * 2 / 5).clamp(28, 84)
    };
    if !pip.is_empty() {
        let mut ph = (pip_w / 3).clamp(8, 26);
        if (ph + 1) * pip.len() as u16 > region_h {
            ph = (region_h / pip.len() as u16).saturating_sub(1).max(4);
        }
        for (i, u) in pip.iter().enumerate() {
            rect_of.insert(
                u.as_u64(),
                Rect {
                    x: scr.x + scr.width.saturating_sub(pip_w + 1),
                    y: top + i as u16 * (ph + 1),
                    width: pip_w,
                    height: ph,
                },
            );
        }
    }
    if !big.is_empty() {
        let bw_total = if pip.is_empty() {
            scr.width
        } else {
            scr.width.saturating_sub(pip_w + 1)
        };
        let cols: usize = if big.len() <= 1 { 1 } else { 2 };
        let rows = big.len().div_ceil(cols);
        let cw = (bw_total / cols as u16).max(1);
        let ch = (region_h / rows as u16).max(3);
        for (i, u) in big.iter().enumerate() {
            let r = (i / cols) as u16;
            let c = (i % cols) as u16;
            rect_of.insert(
                u.as_u64(),
                Rect {
                    x: scr.x + c * cw,
                    y: top + r * ch,
                    width: cw,
                    height: ch,
                },
            );
        }
    }
    for uid in uids.iter() {
        let Some(area) = rect_of.get(&uid.as_u64()).copied() else {
            continue;
        };
        if area.height < 3 || area.width < 6 {
            continue;
        }
        frame.render_widget(Clear, area);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Black))
            .title(format!(" ◉ {uid} · /big {uid} · /unwatch {uid} "));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if let Some(v) = streams.get_mut(&uid.as_u64()) {
            v.set_area(inner);
            if let Some(proto) = v.ready.as_mut() {
                let img = StatefulImage::default().resize(Resize::Fit(None));
                frame.render_stateful_widget(img, inner, proto);
            } else {
                let p = Paragraph::new("loading screen share…")
                    .alignment(ratatui::layout::Alignment::Center);
                frame.render_widget(p, inner);
            }
        } else {
            let p = Paragraph::new("waiting for screen share…")
                .alignment(ratatui::layout::Alignment::Center);
            frame.render_widget(p, inner);
        }
    }
}

fn draw_input(frame: &mut Frame, state: &AppState, area: Rect) {
    let title = match state.current_channel {
        Some(cid) => {
            let typers = if state.prefs.show_typing {
                state.typing_names(cid)
            } else {
                Vec::new()
            };
            if typers.is_empty() {
                if state.editing_message.is_some() {
                    "Editing (Esc to cancel)".to_string()
                } else if let Some(rid) = state.replying_to {
                    let who = state
                        .messages
                        .get(&cid)
                        .and_then(|v| v.iter().find(|m| m.id == rid))
                        .map(|m| m.author.clone())
                        .unwrap_or_default();
                    format!("Replying to {who} (Esc to cancel)")
                } else {
                    "Message".to_string()
                }
            } else if typers.len() == 1 {
                format!("Message — {} is typing…", typers[0])
            } else {
                format!("Message — {} people typing…", typers.len())
            }
        }
        None => "Message".to_string(),
    };
    let border = if state.editing_message.is_some() {
        Style::default().fg(Color::Yellow)
    } else if state.replying_to.is_some() {
        Style::default().fg(Color::Blue)
    } else {
        focused_style(state, Focus::Input)
    };
    let p = Paragraph::new(state.input.as_str()).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border)
            .title(title),
    );
    frame.render_widget(p, area);
}

fn draw_status(frame: &mut Frame, state: &AppState, area: Rect) {
    let voice = if state.voice_status.is_empty() {
        String::new()
    } else {
        format!("🔊 {} · ", state.voice_status)
    };
    let hints = if state.search_open {
        "type to filter · ↑↓ select · Enter open · Esc close"
    } else if state.settings_open {
        "↑↓ move · ←→/Space change · Enter edit text · Esc save & close"
    } else if state.show_help {
        "? or Esc to close help"
    } else if state.editing_message.is_some() {
        "editing · Enter save · Esc cancel"
    } else {
        match state.focus {
            Focus::ServerBar => "←→ servers · Enter · Ctrl+K · m members · , set · ? help",
            Focus::Channels => "↑↓ · Enter · v/V voice · Alt+↑↓ unread · m members · , · ? help",
            Focus::Messages => "↑↓ · e edit · r reply · d del · i img · Ctrl+K · , set · ? help",
            Focus::Input => "type · Enter send · Tab focus · Esc cancel",
        }
    };
    let conn = if state.conn_status.is_empty() {
        "connecting…"
    } else {
        state.conn_status.as_str()
    };
    let conn_color = if conn == "online" {
        Color::Green
    } else {
        Color::Yellow
    };
    let line = Line::from(vec![
        Span::styled(format!(" {conn} "), Style::default().fg(conn_color)),
        Span::styled(
            format!("│ {voice}{} │ {hints} ", state.status_line),
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn rel_time(ts: chrono::DateTime<chrono::Utc>) -> String {
    let secs = (chrono::Utc::now() - ts).num_seconds();
    if secs < 0 {
        return String::new();
    }
    if secs < 45 {
        "· now".into()
    } else if secs < 3600 {
        format!("· {}m", secs / 60)
    } else if secs < 86_400 {
        format!("· {}h", secs / 3600)
    } else {
        format!("· {}d", secs / 86_400)
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}
