pub mod ingest;
pub mod input;
pub mod state;
pub mod stream_view;
pub mod ui;

use std::collections::HashMap;
use std::error::Error;
use std::io;
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{DisableMouseCapture, EnableMouseCapture, EventStream};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures_util::StreamExt;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use tokio::select;

use crate::config::config::Config;
use crate::tui::state::SharedState;
use crate::utils::SnowflakeID;

pub type Protocols = HashMap<SnowflakeID, StatefulProtocol>;

pub async fn run(config: Config) -> Result<(), Box<dyn Error>> {
    crate::utils::set_tui_mode(true);
    crate::utils::set_log_file("dave.log");

    let state = state::AppState::new_shared();

    let (action_tx, action_rx) = tokio::sync::mpsc::unbounded_channel();
    let config_path = std::env::current_dir()
        .map(|p| p.join("config.json"))
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "config.json".to_string());
    {
        let mut s = state.write().await;
        s.gateway_tx = Some(action_tx);
        s.voice_input_device = config.voice_input_device.clone();
        s.voice_output_device = config.voice_output_device.clone();
        s.config_path = config_path.clone();
        s.edit_cfg = state::EditCfg {
            download_media: config.download_media,
            debug: config.debug,
            print_muted_dm: config.print_muted_dm,
            dm_track: config.dm_track,
            server_track: config.server_track,
            track_myself: config.track_myself,
            message_buffer_size: config.message_buffer_size,
            voice_input_device: config.voice_input_device.clone(),
            voice_output_device: config.voice_output_device.clone(),
        };
        if let Ok(txt) = std::fs::read_to_string(&config_path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
                if let Some(ui) = v.get("ui") {
                    let p = &mut s.prefs;
                    if let Some(b) = ui.get("toast_enabled").and_then(|x| x.as_bool()) {
                        p.toast_enabled = b;
                    }
                    if let Some(n) = ui.get("toast_secs").and_then(|x| x.as_u64()) {
                        p.toast_secs = n.clamp(1, 60);
                    }
                    if let Some(b) = ui.get("notify_mentions").and_then(|x| x.as_bool()) {
                        p.notify_mentions = b;
                    }
                    if let Some(b) = ui.get("show_relative_time").and_then(|x| x.as_bool()) {
                        p.show_relative_time = b;
                    }
                    if let Some(b) = ui.get("show_typing").and_then(|x| x.as_bool()) {
                        p.show_typing = b;
                    }
                    if let Some(b) = ui.get("dsp_rx").and_then(|x| x.as_bool()) {
                        p.dsp_rx = b;
                    }
                    if let Some(b) = ui.get("dsp_tx").and_then(|x| x.as_bool()) {
                        p.dsp_tx = b;
                    }
                    if let Some(b) = ui.get("self_mute").and_then(|x| x.as_bool()) {
                        p.self_mute = b;
                    }
                    if let Some(b) = ui.get("self_deaf").and_then(|x| x.as_bool()) {
                        p.self_deaf = b;
                    }
                }
            }
            s.self_mute = s.prefs.self_mute;
            s.self_deaf = s.prefs.self_deaf;
        }
    }

    let gw_state = Arc::clone(&state);
    let gw_config = config.clone();
    let gw_handle = tokio::spawn(async move {
        if let Err(e) = ingest::run_gateway(gw_config, gw_state, action_rx).await {
            eprintln!("gateway loop ended: {e}");
        }
    });

    enable_raw_mode()?;

    let query = Picker::from_query_stdio();
    let queried_ok = query.is_ok();
    let picker = query.unwrap_or_else(|_| Picker::from_fontsize((10, 20)));
    let detected = format!("{:?}", picker.protocol_type());
    let cell = picker.font_size();
    let backend = format!(
        "{detected} {}x{} ({})",
        cell.0,
        cell.1,
        if queried_ok { "queried" } else { "fallback!" }
    );
    crate::info!("image backend: {backend}");
    {
        let mut s = state.write().await;
        s.image_backend = backend.clone();
        s.set_status(format!("image backend: {backend}"));
    }

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let run_result = render_loop(&mut terminal, &state, &config.token, picker).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    gw_handle.abort();
    run_result
}

async fn render_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &SharedState,
    token: &str,
    picker: Picker,
) -> Result<(), Box<dyn Error>> {
    let mut events = EventStream::new();
    let mut protocols: Protocols = HashMap::new();
    let mut stream_gens: HashMap<u64, u64> = HashMap::new();
    let mut stream_views: HashMap<u64, stream_view::StreamView> = HashMap::new();

    loop {
        let watching_active;
        {
            let watching: Vec<u64> = {
                state
                    .read()
                    .await
                    .watching_streams
                    .iter()
                    .map(|s| s.as_u64())
                    .collect()
            };
            watching_active = !watching.is_empty();
            for uid in &watching {
                if let Some((genr, w, h, rgb)) = crate::voice::video::snapshot(*uid) {
                    if stream_gens.get(uid) != Some(&genr) {
                        if let Some(img) = image::RgbImage::from_raw(w, h, rgb) {
                            let dynimg = image::DynamicImage::ImageRgb8(img);
                            stream_views
                                .entry(*uid)
                                .or_insert_with(|| stream_view::StreamView::new(&picker))
                                .update_frame(dynimg);
                            stream_gens.insert(*uid, genr);
                        }
                    }
                }
            }
            for v in stream_views.values_mut() {
                v.poll();
            }
            let stale: Vec<u64> = stream_views
                .keys()
                .copied()
                .filter(|u| !watching.contains(u))
                .collect();
            for u in stale {
                stream_gens.remove(&u);
                stream_views.remove(&u);
            }
        }

        let new_imgs: Vec<(SnowflakeID, image::DynamicImage)> = {
            let snap = state.read().await;
            snap.image_cache
                .iter()
                .filter(|(id, _)| !protocols.contains_key(*id))
                .map(|(id, img)| (*id, img.clone()))
                .collect()
        };
        for (id, img) in new_imgs {
            let proto = picker.new_resize_protocol(img);
            protocols.insert(id, proto);
        }

        {
            let snap = state.read().await;
            if snap.should_quit {
                break;
            }
            terminal.draw(|f| ui::draw(f, &snap, &mut protocols, &mut stream_views))?;
        }

        let period = if watching_active {
            Duration::from_millis(15)
        } else {
            Duration::from_millis(200)
        };
        select! {
            maybe_event = events.next() => {
                match maybe_event {
                    Some(Ok(event)) => {
                        let _ = input::handle_event(event, state, token).await;
                    }
                    Some(Err(e)) => {
                        state.write().await.set_status(format!("term event err: {e}"));
                    }
                    None => break,
                }
            }
            _ = tokio::time::sleep(period) => {}
        }
    }
    Ok(())
}
