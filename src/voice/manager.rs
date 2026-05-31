use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::UdpSocket;
use tokio::select;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tungstenite::Message;

use crate::voice::audio::{self, AudioParams};
use crate::voice::gateway::{self, VoiceEnvelope};
use crate::voice::udp;
use crate::{error, info, warn};

type BoxErr = Box<dyn Error + Send + Sync>;

fn interpret(msg: Message, last_seq: &mut Option<u32>) -> Result<Option<VoiceEnvelope>, BoxErr> {
    use crate::info;
    use crate::voice::dave::wire;

    match msg {
        Message::Text(t) => {
            let s = t.as_str();
            if s.trim().is_empty() {
                return Ok(None);
            }
            match serde_json::from_str::<VoiceEnvelope>(s) {
                Ok(env) => {
                    if let Some(seq) = env.seq {
                        *last_seq = Some(seq as u32);
                    }
                    if (21..=31).contains(&env.op) {
                        info!("voice DAVE op {} d={}", env.op, env.d);
                    }
                    Ok(Some(env))
                }
                Err(_) => Ok(None),
            }
        }
        Message::Binary(b) => {
            if let Some(f) = wire::parse_inbound_binary(&b) {
                if let Some(s) = f.seq {
                    *last_seq = Some(s as u32);
                }
                info!(
                    "voice DAVE bin op={} seq={:?} tid={:?} body={}B",
                    f.opcode,
                    f.seq,
                    f.transition_id,
                    f.body.len()
                );
            } else {
                info!("voice binary frame ({}B, unparsed)", b.len());
            }
            Ok(None)
        }
        Message::Close(frame) => {
            let detail = frame
                .map(|f| format!("code {} ({})", u16::from(f.code), f.reason))
                .unwrap_or_else(|| "no close frame".to_string());
            Err(format!("voice gateway closed: {detail}").into())
        }
        _ => Ok(None),
    }
}

pub enum VoiceCommand {
    Disconnect,
    SetMute(bool),
    SetDeaf(bool),
}

#[derive(Clone, Debug)]
pub struct VoiceSignal {
    pub guild_id: String,
    pub user_id: String,
    pub channel_id: String,
    pub session_id: String,
    pub token: String,
    pub endpoint: String,

    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub dsp_rx: bool,
    pub dsp_tx: bool,

    pub watch_uid: Option<u64>,
    pub server_id_override: Option<String>,
    pub broadcast: bool,
}

pub struct VoiceManager;

impl VoiceManager {
    pub async fn run(
        signal: VoiceSignal,
        mut cmd_rx: mpsc::UnboundedReceiver<VoiceCommand>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let host = signal.endpoint.trim_end_matches('/');
        let base = if host.starts_with("wss://") {
            host.to_string()
        } else {
            format!("wss://{host}")
        };
        let url = if base.contains("?v=") {
            base
        } else {
            format!("{base}/?v=9")
        };
        let conn_tag = match signal.watch_uid {
            Some(u) => format!("watch[{u}]"),
            None => "call".to_string(),
        };
        info!("Voice[{conn_tag}]: connecting to {url}");

        let (ws, _) = connect_async(&url).await?;
        let (mut sink, mut stream) = ws.split();

        let server_id = signal
            .server_id_override
            .clone()
            .unwrap_or_else(|| signal.guild_id.clone());
        if signal.watch_uid.is_some() || signal.broadcast {
            info!(
                "Voice({}): IDENTIFY server_id={server_id} watch_uid={:?}",
                if signal.broadcast {
                    "broadcast"
                } else {
                    "watch"
                },
                signal.watch_uid
            );
            gateway::send_identify_watch(
                &mut sink,
                &server_id,
                &signal.user_id,
                &signal.session_id,
                &signal.token,
            )
            .await?;
        } else {
            gateway::send_identify(
                &mut sink,
                &server_id,
                &signal.user_id,
                &signal.session_id,
                &signal.token,
            )
            .await?;
        }

        let mut last_seq: Option<u32> = None;

        let mut heartbeat_interval: Option<Duration> = None;
        let mut ready: Option<gateway::ReadyD> = None;
        while heartbeat_interval.is_none() || ready.is_none() {
            let Some(msg) = stream.next().await else {
                return Err("voice ws closed during handshake".into());
            };
            let Some(env) = interpret(msg?, &mut last_seq)? else {
                continue;
            };
            match env.op {
                8 => {
                    let h: gateway::HelloD = serde_json::from_value(env.d)?;
                    heartbeat_interval = Some(Duration::from_millis(h.heartbeat_interval as u64));
                }
                2 => {
                    ready = Some(serde_json::from_value::<gateway::ReadyD>(env.d)?);
                }
                _ => {}
            }
        }
        let heartbeat_interval = heartbeat_interval.unwrap();
        let ready = ready.unwrap();
        info!(
            "Voice READY: ssrc={} endpoint={}:{} modes={:?}",
            ready.ssrc, ready.ip, ready.port, ready.modes
        );

        let udp_sock = UdpSocket::bind("0.0.0.0:0").await?;
        let remote: SocketAddr = format!("{}:{}", ready.ip, ready.port).parse()?;
        let discovered = udp::ip_discovery(&udp_sock, ready.ssrc, remote).await?;
        info!(
            "Voice external addr discovered: {}:{}",
            discovered.address, discovered.port
        );

        const PREFERRED_MODE: &str = "aead_aes256_gcm_rtpsize";
        let mode = if ready.modes.iter().any(|m| m == PREFERRED_MODE) {
            PREFERRED_MODE.to_string()
        } else {
            let fb = ready
                .modes
                .first()
                .cloned()
                .unwrap_or_else(|| PREFERRED_MODE.to_string());
            warn!(
                "Voice: {PREFERRED_MODE} not offered ({:?}), using {fb}",
                ready.modes
            );
            fb
        };

        info!(
            "Voice SELECT_PROTOCOL mode={mode} (gateway offered: {:?})",
            ready.modes
        );
        gateway::send_select_protocol(&mut sink, &discovered.address, discovered.port, &mode)
            .await?;

        let mut pending_speakers: std::collections::HashMap<u32, u64> =
            std::collections::HashMap::new();
        let mut roster: std::collections::HashSet<u64> = std::collections::HashSet::new();
        let our_uid_opt = signal.user_id.parse::<u64>().ok();
        let note_speaker =
            |env: &VoiceEnvelope,
             map: &mut std::collections::HashMap<u32, u64>,
             roster: &mut std::collections::HashSet<u64>| {
                if env.op == 11 {
                    if let Some(arr) = env.d.get("user_ids").and_then(|v| v.as_array()) {
                        for u in arr {
                            if let Some(uid) = u.as_str().and_then(|s| s.parse::<u64>().ok()) {
                                roster.insert(uid);
                            }
                        }
                    }
                    return;
                }
                if env.op != 5 {
                    return;
                }
                let uid = env
                    .d
                    .get("user_id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<u64>().ok());
                let ssrc = env.d.get("ssrc").and_then(|v| v.as_u64()).map(|v| v as u32);
                if let Some(uid) = uid {
                    roster.insert(uid);
                    if let Some(ssrc) = ssrc {
                        if Some(uid) != our_uid_opt && ssrc != 0 {
                            map.insert(ssrc, uid);
                        }
                    }
                }
            };

        let session_desc: gateway::SessionDescriptionD;
        loop {
            let Some(msg) = stream.next().await else {
                return Err("voice ws closed before session_description".into());
            };
            let Some(env) = interpret(msg?, &mut last_seq)? else {
                continue;
            };
            note_speaker(&env, &mut pending_speakers, &mut roster);
            if env.op == 5 || env.op == 11 {
                info!("DAVE pre-op4 op{} d={}", env.op, env.d);
            }
            if env.op == 4 {
                session_desc = serde_json::from_value(env.d)?;
                break;
            }
        }
        if session_desc.secret_key.len() != 32 {
            return Err(format!(
                "expected 32-byte secret_key, got {}",
                session_desc.secret_key.len()
            )
            .into());
        }
        let mut secret_key = [0u8; 32];
        secret_key.copy_from_slice(&session_desc.secret_key);
        info!(
            "Voice SESSION_DESCRIPTION received (mode={})",
            session_desc.mode
        );

        let our_uid = signal.user_id.parse::<u64>().unwrap_or(0);
        let dave_group_id = signal.channel_id.parse::<u64>().unwrap_or(0);
        info!("DAVE: group_id (voice channel) = {dave_group_id}");
        let rt = crate::voice::dave::runtime::DaveRt::new(our_uid, ready.ssrc);
        if let Some(wu) = signal.watch_uid {
            rt.lock().unwrap().set_watch_uid(wu);
            info!("DAVE(watch): broadcaster uid={wu}, group_id={dave_group_id}");
            if let Err(e) = gateway::send_video_sink_wants(&mut sink, ready.ssrc).await {
                error!("watch: op12 video sink wants: {e}");
            }
        }
        if signal.broadcast {
            rt.lock().unwrap().set_broadcast();
            info!(
                "DAVE(broadcast): video ssrc={} group_id={dave_group_id}",
                ready.ssrc
            );
            if let Err(e) = gateway::send_video_sink_wants(&mut sink, ready.ssrc).await {
                error!("broadcast: op12: {e}");
            }
        }

        let running = Arc::new(AtomicBool::new(true));
        let mute = Arc::new(AtomicBool::new(false));
        let deaf = Arc::new(AtomicBool::new(false));
        let rekey = Arc::new(AtomicBool::new(false));
        let socket = Arc::new(udp_sock);
        let (send_h, recv_h) = audio::spawn_pipeline(
            socket.clone(),
            AudioParams {
                ssrc: ready.ssrc,
                remote,
                secret_key,
                running: running.clone(),
                mute: mute.clone(),
                deaf: deaf.clone(),
                rekey: rekey.clone(),
                video_only: signal.watch_uid.is_some() || signal.broadcast,
                input_device: signal.input_device.clone(),
                output_device: signal.output_device.clone(),
                dsp_rx: crate::voice::dsp::DspParams {
                    enabled: signal.dsp_rx,
                    ..Default::default()
                },
                dsp_tx: crate::voice::dsp::DspParams {
                    enabled: signal.dsp_tx,
                    ..Default::default()
                },
                dave: rt.clone(),
            },
        )?;

        if signal.broadcast {
            use aes_gcm::aead::KeyInit;
            match aes_gcm::Aes256Gcm::new_from_slice(&secret_key) {
                Ok(vc) => {
                    crate::voice::vsend::spawn_broadcast(
                        socket.clone(),
                        remote,
                        vc,
                        ready.ssrc,
                        rt.clone(),
                        running.clone(),
                        15,
                    );
                    let _ = gateway::send_speaking(&mut sink, 2, ready.ssrc).await;
                    info!("broadcast: vsend pipeline spawned");
                }
                Err(e) => error!("broadcast: cipher init: {e}"),
            }
        }

        let mut hb = tokio::time::interval(heartbeat_interval);
        let mut rkck = tokio::time::interval(Duration::from_millis(750));
        let mut nonce = 0u64;

        let mut shutdown_reason: &'static str = "voice loop ended";
        loop {
            select! {
                _ = hb.tick() => {
                    if let Err(e) = gateway::send_heartbeat(&mut sink, nonce, last_seq).await {
                        error!("voice[{conn_tag}] heartbeat: {e}");
                        shutdown_reason = "heartbeat send failed";
                        break;
                    }
                    nonce = nonce.wrapping_add(1);
                }
                _ = rkck.tick() => {
                    if rekey.swap(false, Ordering::Relaxed) {
                        let kp = rt.lock().unwrap().rebootstrap();
                        match kp {
                            Some(kp) => {
                                use crate::voice::dave::wire::op as davop;
                                let pkt = crate::voice::dave::wire::frame_outbound(
                                    davop::MLS_KEY_PACKAGE,
                                    &kp,
                                );
                                info!(
                                    "DAVE[{conn_tag}]: re-key recovery → resending op26 key package ({}B)",
                                    pkt.len()
                                );
                                if let Err(e) =
                                    sink.send(Message::Binary(pkt.into())).await
                                {
                                    error!("DAVE[{conn_tag}]: re-key op26 send: {e}");
                                }
                            }
                            None => {
                                error!("DAVE[{conn_tag}]: re-key rebootstrap failed (no key package)");
                            }
                        }
                    }
                }
                msg = stream.next() => {
                    let Some(msg) = msg else {
                        shutdown_reason = "voice ws closed";
                        break;
                    };
                    match msg {
                        Ok(Message::Binary(b)) => {

                            {
                                let head: Vec<String> =
                                    b.iter().take(48).map(|x| format!("{x:02x}")).collect();
                                info!("DAVE rx bin {}B head={}", b.len(), head.join(""));
                            }

                            if let Some(f) = crate::voice::dave::wire::parse_inbound_binary(&b) {
                                if let Some(s) = f.seq {
                                    last_seq = Some(s as u32);
                                }
                                use crate::voice::dave::wire::op as davop;
                                let recognized: Vec<u64> = roster
                                    .iter()
                                    .copied()
                                    .chain(std::iter::once(our_uid))
                                    .collect();
                                match f.opcode {
                                    davop::MLS_EXTERNAL_SENDER => {
                                        let kp = {
                                            let mut g = rt.lock().unwrap();
                                            g.on_external_sender(&f.body, dave_group_id)
                                        };
                                        info!(
                                            "DAVE: op25 external sender stored ({}B)",
                                            f.body.len()
                                        );
                                        if let Some(kp) = kp {
                                            let pkt = crate::voice::dave::wire::frame_outbound(
                                                davop::MLS_KEY_PACKAGE,
                                                &kp,
                                            );
                                            info!("DAVE: op26 {}B sent (libdave)", pkt.len());
                                            if let Err(e) =
                                                sink.send(Message::Binary(pkt.into())).await
                                            {
                                                error!("DAVE: op26 send: {e}");
                                            }
                                        } else {
                                            error!("DAVE: op25 → no key package");
                                        }
                                    }
                                    davop::MLS_PROPOSALS => {
                                        let op28 = {
                                            let mut g = rt.lock().unwrap();
                                            g.on_proposals_get_op28(&f.body, &recognized)
                                        };
                                        match op28 {
                                            Some(op28) => {
                                                let pkt = crate::voice::dave::wire::frame_outbound(
                                                    davop::MLS_COMMIT_WELCOME,
                                                    &op28,
                                                );
                                                info!(
                                                    "DAVE: op27 ({}B) → op28 {}B sent",
                                                    f.body.len(),
                                                    pkt.len()
                                                );
                                                if let Err(e) =
                                                    sink.send(Message::Binary(pkt.into())).await
                                                {
                                                    error!("DAVE: op28 send: {e}");
                                                }
                                            }
                                            None => {
                                                info!("DAVE: op27 processed, no commit produced")
                                            }
                                        }
                                    }
                                    davop::MLS_ANNOUNCE_COMMIT => {
                                        let ok = rt.lock().unwrap().on_commit(&f.body);
                                        let tid = f.transition_id.unwrap_or(0);
                                        let _ = gateway::send_ready_for_transition(&mut sink, tid)
                                            .await;
                                        {
                                            let mut g = rt.lock().unwrap();
                                            g.arm(&pending_speakers);
                                            info!(
                                                "DAVE: op29 ok={ok} → op23(tid={tid}) armed verif={}",
                                                g.verification_code()
                                                    .unwrap_or_else(|| "n/a".into())
                                            );
                                        }
                                        let _ =
                                            gateway::send_speaking(&mut sink, 1, ready.ssrc).await;
                                    }
                                    davop::MLS_WELCOME => {
                                        let ok = {
                                            let mut g = rt.lock().unwrap();
                                            g.on_welcome(&f.body, &recognized)
                                        };
                                        let tid = f.transition_id.unwrap_or(0);
                                        let _ = gateway::send_ready_for_transition(&mut sink, tid)
                                            .await;
                                        {
                                            let mut g = rt.lock().unwrap();
                                            g.arm(&pending_speakers);
                                            info!(
                                                "DAVE: op30 ok={ok} → op23(tid={tid}) armed verif={}",
                                                g.verification_code()
                                                    .unwrap_or_else(|| "n/a".into())
                                            );
                                            info!(
                                                "DAVE state: pending_speakers={} roster={} {}",
                                                pending_speakers.len(),
                                                roster.len(),
                                                g.debug_state()
                                            );
                                        }
                                        let _ =
                                            gateway::send_speaking(&mut sink, 1, ready.ssrc).await;
                                    }
                                    other => {
                                        info!(
                                            "DAVE bin op={} seq={:?} tid={:?} body={}B (unhandled)",
                                            other, f.seq, f.transition_id, f.body.len()
                                        );
                                    }
                                }
                            }
                        }
                        Ok(m) => {
                            match interpret(m, &mut last_seq) {
                                Err(e) => {
                                    error!("voice[{conn_tag}]: {e}");
                                    shutdown_reason = "voice gateway closed";
                                    break;
                                }
                                Ok(Some(env)) if env.op == 22 => {
                                    {
                                        let mut g = rt.lock().unwrap();
                                        g.arm(&pending_speakers);
                                        info!(
                                            "DAVE: op22 execute → armed verif={}",
                                            g.verification_code().unwrap_or_else(|| "n/a".into())
                                        );
                                        info!(
                                            "DAVE state: pending_speakers={} roster={} {}",
                                            pending_speakers.len(),
                                            roster.len(),
                                            g.debug_state()
                                        );
                                    }
                                    let _ = gateway::send_speaking(&mut sink, 1, ready.ssrc).await;
                                }
                                Ok(Some(env)) if env.op == 12 => {
                                    let uid = env.d.get("user_id").and_then(|v| v.as_str())
                                        .and_then(|s| s.parse::<u64>().ok());
                                    let vssrc = env.d.get("video_ssrc")
                                        .and_then(|v| v.as_u64()).map(|v| v as u32);
                                    let assrc = env.d.get("audio_ssrc")
                                        .and_then(|v| v.as_u64()).map(|v| v as u32);
                                    if let Some(uid) = uid {
                                        if Some(uid) != our_uid_opt {
                                            let mut g = rt.lock().unwrap();
                                            if let Some(a) = assrc.filter(|s| *s != 0) {
                                                g.note_ssrc_uid(a, uid);
                                            }
                                            if let Some(v) = vssrc.filter(|s| *s != 0) {
                                                g.note_ssrc_uid(v, uid);
                                            }
                                            info!("DAVE: op12 video uid={uid} audio_ssrc={assrc:?} video_ssrc={vssrc:?} → bound");
                                        }
                                    }
                                }
                                Ok(Some(env)) if env.op == 5 => {
                                    note_speaker(&env, &mut pending_speakers, &mut roster);
                                    let uid = env.d.get("user_id").and_then(|v| v.as_str())
                                        .and_then(|s| s.parse::<u64>().ok());
                                    let rssrc = env.d.get("ssrc").and_then(|v| v.as_u64())
                                        .map(|v| v as u32);
                                    if let (Some(uid), Some(rssrc)) = (uid, rssrc) {
                                        if Some(uid) != our_uid_opt {
                                            rt.lock().unwrap().note_ssrc_uid(rssrc, uid);
                                            info!("DAVE: op5 speaking uid={uid} ssrc={rssrc} → recv ratchet bound");
                                        }
                                    }
                                }
                                Ok(_) => {}
                            }
                        }
                        Err(e) => {
                            error!("voice[{conn_tag}] ws error: {e}");
                            shutdown_reason = "voice ws error";
                            break;
                        }
                    }
                }
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(VoiceCommand::Disconnect) => {
                            info!("Voice[{conn_tag}]: explicit Disconnect command received");
                            shutdown_reason = "disconnect requested";
                            break;
                        }
                        None => {
                            info!("Voice[{conn_tag}]: cmd channel dropped");
                            shutdown_reason = "cmd channel closed";
                            break;
                        }
                        Some(VoiceCommand::SetMute(m)) => {
                            mute.store(m, Ordering::Relaxed);
                            let _ = gateway::send_speaking(
                                &mut sink,
                                if m { 0 } else { 1 },
                                ready.ssrc,
                            ).await;
                        }
                        Some(VoiceCommand::SetDeaf(d)) => {
                            deaf.store(d, Ordering::Relaxed);
                        }
                    }
                }
            }
        }

        running.store(false, Ordering::Relaxed);
        send_h.abort();
        recv_h.abort();
        let _ = sink.close().await;
        info!("Voice[{conn_tag}] manager exiting: {shutdown_reason}");
        Ok(())
    }
}
