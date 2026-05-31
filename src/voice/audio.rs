use std::collections::VecDeque;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use byteorder::{BigEndian, ByteOrder};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use crate::voice::dave::runtime::SharedRt;
use crate::voice::udp::make_rtp_header;

fn aead_nonce(counter: u32) -> [u8; 12] {
    let mut n = [0u8; 12];
    BigEndian::write_u32(&mut n[0..4], counter);
    n
}

fn transport_split(pkt: &[u8]) -> Option<(usize, usize)> {
    if pkt.len() < 12 {
        return None;
    }
    let cc = (pkt[0] & 0x0F) as usize;
    let base = 12 + 4 * cc;
    if pkt[0] & 0x10 != 0 {
        if pkt.len() < base + 4 {
            return None;
        }
        let ext_words = u16::from_be_bytes([pkt[base + 2], pkt[base + 3]]) as usize;
        let ext_body = ext_words * 4;
        Some((base + 4, ext_body))
    } else {
        if pkt.len() < base {
            return None;
        }
        Some((base, 0))
    }
}

const HFP_RATE: u32 = 16_000;

#[derive(Default)]
struct SsrcBuf {
    buf: VecDeque<f32>,
    started: bool,
}
type Mix = Arc<Mutex<std::collections::HashMap<u32, SsrcBuf>>>;

pub type BoxErr = Box<dyn Error + Send + Sync>;

pub const SAMPLE_RATE: u32 = 48_000;
pub const CHANNELS: u16 = 1;
pub const FRAME_MS: u32 = 20;
pub const FRAME_SAMPLES: usize = (SAMPLE_RATE / 1000 * FRAME_MS) as usize;

pub struct AudioParams {
    pub ssrc: u32,
    pub remote: SocketAddr,
    pub secret_key: [u8; 32],
    pub running: Arc<AtomicBool>,
    pub mute: Arc<AtomicBool>,
    pub deaf: Arc<AtomicBool>,
    pub rekey: Arc<AtomicBool>,
    pub video_only: bool,

    pub input_device: Option<String>,
    pub output_device: Option<String>,

    pub dsp_rx: crate::voice::dsp::DspParams,
    pub dsp_tx: crate::voice::dsp::DspParams,

    pub dave: SharedRt,
}

fn pick_input(host: &cpal::Host, want: &Option<String>) -> Option<cpal::Device> {
    use crate::info;
    use cpal::traits::HostTrait;
    if let Some(want) = want {
        if let Ok(devs) = host.input_devices() {
            for d in devs {
                if d.name()
                    .map(|n| n.to_lowercase().contains(&want.to_lowercase()))
                    .unwrap_or(false)
                {
                    info!("voice: input device → {}", d.name().unwrap_or_default());
                    return Some(d);
                }
            }
        }
        info!("voice: input '{want}' not found, using default");
    }
    host.default_input_device()
}

fn pick_output(host: &cpal::Host, want: &Option<String>) -> Option<cpal::Device> {
    use crate::info;
    use cpal::traits::HostTrait;
    if let Some(want) = want {
        let needle = want.to_lowercase();
        if let Ok(devs) = host.output_devices() {
            for d in devs {
                if d.name()
                    .map(|n| n.to_lowercase().contains(&needle))
                    .unwrap_or(false)
                {
                    info!("voice: output device → {}", d.name().unwrap_or_default());
                    return Some(d);
                }
            }
        }

        if let Ok(devs) = host.input_devices() {
            for d in devs {
                if d.name()
                    .map(|n| n.to_lowercase().contains(&needle))
                    .unwrap_or(false)
                {
                    info!(
                        "voice: output '{want}' only on input list (BT/HFP); trying it for output → {}",
                        d.name().unwrap_or_default()
                    );
                    return Some(d);
                }
            }
        }
        info!("voice: output '{want}' not found, using default");
    }
    host.default_output_device()
}

pub fn spawn_pipeline(
    socket: Arc<UdpSocket>,
    params: AudioParams,
) -> Result<(tokio::task::JoinHandle<()>, tokio::task::JoinHandle<()>), BoxErr> {
    let cipher = Aes256Gcm::new_from_slice(&params.secret_key)
        .map_err(|e| format!("aes-256-gcm key init: {e}"))?;

    {
        use crate::info;
        use cpal::traits::HostTrait;
        let host = cpal::default_host();
        if let Ok(ins) = host.input_devices() {
            let names: Vec<String> = ins.filter_map(|d| d.name().ok()).collect();
            info!("voice: input devices = {names:?}");
        }
        if let Ok(outs) = host.output_devices() {
            let names: Vec<String> = outs.filter_map(|d| d.name().ok()).collect();
            info!("voice: output devices = {names:?}");
        }
    }

    let playback: Mix = Arc::new(Mutex::new(std::collections::HashMap::new()));

    let send_handle = if params.video_only {
        crate::info!("voice: video-only pipeline (no mic/playback)");
        tokio::spawn(async {})
    } else {
        let (cap_tx, cap_rx) = mpsc::channel::<Vec<f32>>(64);
        spawn_capture_thread(cap_tx, params.input_device.clone());
        spawn_playback_thread(
            playback.clone(),
            params.output_device.clone(),
            params.dsp_rx.clone(),
            params.deaf.clone(),
        );
        tokio::spawn(send_loop(
            socket.clone(),
            params.remote,
            params.ssrc,
            cipher.clone(),
            params.running.clone(),
            cap_rx,
            params.dave.clone(),
            params.dsp_tx.clone(),
            params.mute.clone(),
        ))
    };
    let recv_handle = tokio::spawn(recv_loop(
        socket,
        cipher,
        params.running.clone(),
        playback,
        params.dave.clone(),
        params.rekey.clone(),
    ));
    Ok((send_handle, recv_handle))
}

fn build_capture_stream(
    device: &cpal::Device,
    tx: mpsc::Sender<Vec<f32>>,
    peak_bits: Arc<std::sync::atomic::AtomicU32>,
) -> Option<cpal::Stream> {
    use crate::info;
    use std::sync::atomic::Ordering;
    let (in_rate, in_ch, config): (u32, usize, cpal::StreamConfig) =
        match device.default_input_config() {
            Ok(c) => (c.sample_rate().0, c.channels() as usize, c.into()),
            Err(_) => (
                HFP_RATE,
                1,
                cpal::StreamConfig {
                    channels: 1,
                    sample_rate: cpal::SampleRate(HFP_RATE),
                    buffer_size: cpal::BufferSize::Default,
                },
            ),
        };
    info!(
        "voice: trying capture device '{}' {} Hz x{} ch",
        device.name().unwrap_or_else(|_| "?".into()),
        in_rate,
        in_ch
    );
    let ratio = in_rate as f64 / SAMPLE_RATE as f64;
    let mut backlog: Vec<f32> = Vec::with_capacity(in_rate as usize);
    let mut pos = 0f64;
    let mut frame: Vec<f32> = Vec::with_capacity(FRAME_SAMPLES);
    let stream = device
        .build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mut mx = 0u32;
                for &x in data {
                    let b = x.abs().to_bits();
                    if b > mx {
                        mx = b;
                    }
                }
                peak_bits.fetch_max(mx, Ordering::Relaxed);
                for chunk in data.chunks(in_ch.max(1)) {
                    let m = chunk.iter().sum::<f32>() / in_ch.max(1) as f32;
                    backlog.push(m);
                }
                while (pos as usize) + 1 < backlog.len() {
                    let i = pos as usize;
                    let frac = (pos - i as f64) as f32;
                    let s = backlog[i] * (1.0 - frac) + backlog[i + 1] * frac;
                    frame.push(s);
                    if frame.len() == FRAME_SAMPLES {
                        let _ = tx.try_send(std::mem::replace(
                            &mut frame,
                            Vec::with_capacity(FRAME_SAMPLES),
                        ));
                    }
                    pos += ratio;
                }
                let consumed = pos as usize;
                if consumed > 0 && consumed <= backlog.len() {
                    backlog.drain(..consumed);
                    pos -= consumed as f64;
                }
            },
            |e| {
                use crate::error;
                error!("voice cpal input error: {e:?}");
            },
            None,
        )
        .ok()?;
    stream.play().ok()?;
    Some(stream)
}

fn spawn_capture_thread(tx: mpsc::Sender<Vec<f32>>, want: Option<String>) {
    use crate::{error, info};
    use std::sync::atomic::{AtomicU32, Ordering};
    std::thread::spawn(move || {
        let host = cpal::default_host();
        let mut candidates: Vec<cpal::Device> = Vec::new();
        if let Some(w) = &want {
            if let Ok(devs) = host.input_devices() {
                for d in devs {
                    if d.name()
                        .map(|n| n.to_lowercase().contains(&w.to_lowercase()))
                        .unwrap_or(false)
                    {
                        candidates.push(d);
                    }
                }
            }
        }
        if let Some(d) = host.default_input_device() {
            candidates.push(d);
        }
        if let Ok(devs) = host.input_devices() {
            for d in devs {
                candidates.push(d);
            }
        }
        let mut seen = std::collections::HashSet::new();
        candidates.retain(|d| {
            let n = d.name().unwrap_or_default();
            seen.insert(n)
        });
        if candidates.is_empty() {
            error!("voice: no input device");
            return;
        }

        let last = candidates.len() - 1;
        for (idx, dev) in candidates.into_iter().enumerate() {
            let name = dev.name().unwrap_or_else(|_| "?".into());
            let peak = Arc::new(AtomicU32::new(0));
            let Some(stream) = build_capture_stream(&dev, tx.clone(), peak.clone()) else {
                continue;
            };
            std::thread::sleep(std::time::Duration::from_millis(1500));
            let pk = f32::from_bits(peak.load(Ordering::Relaxed));
            if pk > 1.0e-5 || idx == last {
                info!("voice: capture device = {name} (peak={pk:.5}) — using it");
                if pk <= 1.0e-5 {
                    error!(
                        "voice: ALL inputs are digital-silent — macOS Microphone permission is almost certainly DENIED for this process. Grant it in System Settings → Privacy & Security → Microphone (add your terminal/app) then fully restart."
                    );
                }
                std::mem::forget(stream);
                loop {
                    std::thread::park();
                }
            }
            info!("voice: capture device '{name}' is silent (peak={pk:.5}), trying next");
            drop(stream);
        }
    });
}

fn spawn_playback_thread(
    ring: Mix,
    want: Option<String>,
    dsp: crate::voice::dsp::DspParams,
    deaf: Arc<AtomicBool>,
) {
    use crate::{error, info};
    std::thread::spawn(move || {
        let host = cpal::default_host();
        let Some(mut device) = pick_output(&host, &want) else {
            error!("voice: no output device — playback disabled");
            return;
        };

        let (out_rate, out_ch, config): (u32, usize, cpal::StreamConfig) =
            match device.default_output_config() {
                Ok(c) => (c.sample_rate().0, c.channels() as usize, c.into()),
                Err(e) => {
                    error!(
                        "voice: '{}' has no output config ({e}); using system default output",
                        device.name().unwrap_or_default()
                    );
                    match host
                        .default_output_device()
                        .and_then(|d| d.default_output_config().ok().map(|c| (d, c)))
                    {
                        Some((d, c)) => {
                            device = d;
                            (c.sample_rate().0, c.channels() as usize, c.into())
                        }
                        None => {
                            error!("voice: no usable output device — playback disabled");
                            return;
                        }
                    }
                }
            };
        info!(
            "voice: speaker playback {} Hz x{} ch ← resample from {} Hz mono",
            out_rate, out_ch, SAMPLE_RATE
        );

        let ratio = SAMPLE_RATE as f64 / out_rate as f64;
        let prebuf = (SAMPLE_RATE as usize / 1000) * 40;
        let mut pos = 0f64;
        let mix_cb = ring.clone();
        info!(
            "voice: RX DSP enabled={} hpf={}@{} gate={} comp={}(r{}) agc={}",
            dsp.enabled, dsp.hpf, dsp.hpf_hz, dsp.gate, dsp.comp, dsp.comp_ratio, dsp.agc
        );
        let mut chain = crate::voice::dsp::DspChain::new(SAMPLE_RATE);
        let mut scratch: Vec<f32> = Vec::new();
        let stream = device.build_output_stream(
            &config,
            move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                if deaf.load(Ordering::Relaxed) {
                    for s in out.iter_mut() {
                        *s = 0.0;
                    }
                    return;
                }
                let ch = out_ch.max(1);
                let frames = out.len() / ch;
                scratch.clear();
                scratch.resize(frames, 0.0);
                {
                    let mut m = mix_cb.lock().unwrap();
                    for fo in scratch.iter_mut() {
                        let adv = pos as usize;
                        let frac = (pos - adv as f64) as f32;
                        let mut sum = 0.0f32;
                        for sb in m.values_mut() {
                            if !sb.started {
                                if sb.buf.len() >= prebuf {
                                    sb.started = true;
                                } else {
                                    continue;
                                }
                            }
                            for _ in 0..adv {
                                sb.buf.pop_front();
                            }
                            let s = match (sb.buf.front().copied(), sb.buf.get(1).copied()) {
                                (Some(a), Some(b)) => a * (1.0 - frac) + b * frac,
                                (Some(a), None) => a,
                                _ => 0.0,
                            };
                            sum += s;
                        }
                        pos = pos - adv as f64 + ratio;
                        *fo = sum;
                    }
                }
                chain.process(&dsp, &mut scratch);
                for (fi, frame) in out.chunks_mut(ch).enumerate() {
                    let v = scratch.get(fi).copied().unwrap_or(0.0).clamp(-1.0, 1.0);
                    for (i, slot) in frame.iter_mut().enumerate() {
                        *slot = if i < 2 { v } else { 0.0 };
                    }
                }
            },
            |e| {
                use crate::error;
                error!("voice cpal output error: {e:?}");
            },
            None,
        );
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                error!("voice: output stream build failed: {e}");
                return;
            }
        };
        if let Err(e) = stream.play() {
            error!("voice: output stream play failed: {e}");
            return;
        }
        std::mem::forget(stream);
        loop {
            std::thread::park();
        }
    });
}

async fn send_loop(
    socket: Arc<UdpSocket>,
    remote: SocketAddr,
    ssrc: u32,
    cipher: Aes256Gcm,
    running: Arc<AtomicBool>,
    mut rx: mpsc::Receiver<Vec<f32>>,
    dave: SharedRt,
    dsp_tx: crate::voice::dsp::DspParams,
    mute: Arc<AtomicBool>,
) {
    crate::info!(
        "voice: TX DSP enabled={} hpf={} gate={} comp={} agc={}",
        dsp_tx.enabled,
        dsp_tx.hpf,
        dsp_tx.gate,
        dsp_tx.comp,
        dsp_tx.agc
    );
    let mut tx_chain = crate::voice::dsp::DspChain::new(SAMPLE_RATE);
    let channels = if CHANNELS == 1 {
        opus::Channels::Mono
    } else {
        opus::Channels::Stereo
    };
    let mut opus = match opus::Encoder::new(SAMPLE_RATE, channels, opus::Application::Voip) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("voice: opus encoder: {e}");
            return;
        }
    };
    let _ = opus.set_bitrate(opus::Bitrate::Bits(64_000));

    let mut seq: u16 = 0;
    let mut timestamp: u32 = 0;
    let mut nonce_counter: u32 = 0;
    let mut opus_buf = vec![0u8; 1500];
    let mut send_n: u64 = 0;

    while running.load(Ordering::Relaxed) {
        let Some(mut frame) = rx.recv().await else {
            break;
        };
        if mute.load(Ordering::Relaxed) {
            continue;
        }
        tx_chain.process(&dsp_tx, &mut frame);
        let pcm: Vec<i16> = frame
            .iter()
            .map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .collect();
        let rms = if pcm.is_empty() {
            0.0
        } else {
            let sum: f64 = pcm.iter().map(|&v| (v as f64) * (v as f64)).sum();
            (sum / pcm.len() as f64).sqrt()
        };
        let written = match opus.encode(&pcm, &mut opus_buf) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("voice: opus encode: {e}");
                continue;
            }
        };

        let (media, enc_ok, enabled): (Vec<u8>, bool, bool) = {
            let g = dave.lock().unwrap();
            let en = g.enabled;
            if en {
                match g.encrypt(&opus_buf[..written]) {
                    Some(v) => (v, true, true),
                    None => (opus_buf[..written].to_vec(), false, true),
                }
            } else {
                (opus_buf[..written].to_vec(), false, false)
            }
        };
        send_n += 1;
        if send_n <= 8 || send_n % 250 == 0 {
            crate::info!(
                "voice send #{send_n}: enabled={enabled} dave_enc={enc_ok} rms={rms:.0} opus={written}B media={}B",
                media.len()
            );
        }

        let header = make_rtp_header(seq, timestamp, ssrc);

        let nonce = aead_nonce(nonce_counter);
        let ciphertext = match cipher.encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &media,
                aad: &header,
            },
        ) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("voice: encrypt: {e}");
                continue;
            }
        };

        let mut packet = Vec::with_capacity(12 + ciphertext.len() + 4);
        packet.extend_from_slice(&header);
        packet.extend_from_slice(&ciphertext);
        let mut tail = [0u8; 4];
        BigEndian::write_u32(&mut tail, nonce_counter);
        packet.extend_from_slice(&tail);

        if let Err(e) = socket.send_to(&packet, remote).await {
            eprintln!("voice: udp send: {e}");
        }

        seq = seq.wrapping_add(1);
        timestamp = timestamp.wrapping_add(FRAME_SAMPLES as u32);
        nonce_counter = nonce_counter.wrapping_add(1);
    }
}

async fn recv_loop(
    socket: Arc<UdpSocket>,
    cipher: Aes256Gcm,
    running: Arc<AtomicBool>,
    ring: Mix,
    dave: SharedRt,
    rekey: Arc<AtomicBool>,
) {
    let mut dave_fail_streak: u32 = 0;
    let channels = if CHANNELS == 1 {
        opus::Channels::Mono
    } else {
        opus::Channels::Stereo
    };
    let mut decoders: std::collections::HashMap<u32, opus::Decoder> =
        std::collections::HashMap::new();
    let mut vrx: std::collections::HashMap<u32, crate::voice::video::VideoRx> =
        std::collections::HashMap::new();
    let mut vdec: std::collections::HashMap<u32, std::sync::mpsc::Sender<(u64, Vec<u8>)>> =
        std::collections::HashMap::new();
    let mut vid_rx = 0u64;

    let mut buf = vec![0u8; 4096];
    let mut pcm = vec![0i16; FRAME_SAMPLES * 4];
    let (mut rx_total, mut tp_ok, mut tp_fail, mut dave_ok, mut dave_fail, mut dec_ok) =
        (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let mut armed_diag = 0u32;
    while running.load(Ordering::Relaxed) {
        let n = match socket.recv(&mut buf).await {
            Ok(n) => n,
            Err(_) => continue,
        };
        if n < 12 + 16 + 4 {
            continue;
        }
        rx_total += 1;
        if rx_total % 250 == 0 {
            crate::info!(
                "voice recv: rx={rx_total} transport ok={tp_ok} fail={tp_fail} dave ok={dave_ok} fail={dave_fail} opus_ok={dec_ok}"
            );
        }

        let (split, ext_body) = match transport_split(&buf[..n]) {
            Some((h, e)) if h + 16 + 4 <= n => (h, e),
            _ => {
                tp_fail += 1;
                continue;
            }
        };
        let header = &buf[..split];
        let ssrc = BigEndian::read_u32(&buf[8..12]);
        let pt = buf[1] & 0x7F;
        let is_video = pt == 105;
        let marker = buf[1] & 0x80 != 0;
        let rtp_seq = u16::from_be_bytes([buf[2], buf[3]]);
        let rtp_ts = BigEndian::read_u32(&buf[4..8]);
        let mut nonce = [0u8; 12];
        nonce[..4].copy_from_slice(&buf[n - 4..n]);
        let ct_and_tag = &buf[split..n - 4];
        let decrypted = match cipher.decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: ct_and_tag,
                aad: header,
            },
        ) {
            Ok(p) => {
                tp_ok += 1;
                p
            }
            Err(_) => {
                tp_fail += 1;
                continue;
            }
        };
        if decrypted.len() < ext_body {
            continue;
        }
        let mut plain = decrypted[ext_body..].to_vec();
        if buf[0] & 0x20 != 0 {
            if let Some(&pad) = plain.last() {
                let pad = pad as usize;
                if pad > 0 && pad <= plain.len() {
                    let keep = plain.len() - pad;
                    plain.truncate(keep);
                }
            }
        }

        if is_video {
            vid_rx += 1;
            let uid = {
                let mut g = dave.lock().unwrap();
                g.video_uid(ssrc)
            };
            if !vrx.contains_key(&ssrc) {
                match crate::voice::video::VideoRx::new() {
                    Some(r) => {
                        vrx.insert(ssrc, r);
                    }
                    None => {
                        crate::error!("voice: openh264 decoder init failed");
                        continue;
                    }
                }
            }
            let assembled = vrx
                .get_mut(&ssrc)
                .and_then(|rx| rx.push(rtp_seq, rtp_ts, marker, &plain));
            let Some(asm) = assembled else { continue };
            let dec = {
                let mut g = dave.lock().unwrap();
                g.decrypt_video(ssrc, &asm)
            };
            match &dec {
                Ok(_) => dave_fail_streak = 0,
                Err(_) => {
                    dave_fail_streak += 1;
                    if dave_fail_streak == 240 {
                        crate::info!(
                            "DAVE(video): {dave_fail_streak} consecutive decrypt failures → requesting re-key"
                        );
                        rekey.store(true, Ordering::Relaxed);
                    }
                }
            }
            if vid_rx % 200 == 1 || (vid_rx < 4000 && dec.is_ok()) {
                let st = match &dec {
                    Ok(d) => {
                        let head: Vec<String> =
                            d.iter().take(8).map(|x| format!("{x:02x}")).collect();
                        format!(
                            "DECRYPT ok asm={} clear={} head={}",
                            asm.len(),
                            d.len(),
                            head.join("")
                        )
                    }
                    Err(c) => format!("err rc={c} asm={}", asm.len()),
                };
                crate::info!("voice recv video: rx={vid_rx} ssrc={ssrc} uid={uid:?} {st}");
            }
            if let (Ok(clear), Some(uid)) = (dec, uid) {
                let tx = vdec
                    .entry(ssrc)
                    .or_insert_with(crate::voice::video::spawn_decoder);
                if tx.send((uid, clear)).is_ok() {
                    crate::voice::video::note_recv_au();
                }
            }
            continue;
        }

        let (dav, armed_here) = {
            let mut g = dave.lock().unwrap();
            let has = g.has_decryptor(ssrc);
            let r = g.decrypt(ssrc, &plain);
            if has && armed_diag < 20 {
                armed_diag += 1;
                let rc = match &r {
                    Ok(o) => format!("OK len={}", o.len()),
                    Err(c) => format!("ERR rc={c}"),
                };
                crate::info!(
                    "DAVE diag recv(armed): ssrc={ssrc} plen={} -> {rc}",
                    plain.len()
                );
            }
            (r, has)
        };
        let opus: Vec<u8> = match dav {
            Ok(o) => {
                dave_ok += 1;
                if armed_here {
                    dave_fail_streak = 0;
                }
                o
            }
            Err(_) => {
                if armed_here {
                    dave_fail_streak += 1;
                    if dave_fail_streak == 240 {
                        crate::info!(
                            "DAVE: {dave_fail_streak} consecutive decrypt failures while armed → requesting re-key"
                        );
                        rekey.store(true, Ordering::Relaxed);
                    }
                }
                let is_protocol = plain.len() >= 3 && plain[plain.len() - 2..] == [0xFA, 0xFA];
                if is_protocol {
                    dave_fail += 1;
                    continue;
                }
                plain
            }
        };

        let decoder = match decoders.get_mut(&ssrc) {
            Some(d) => d,
            None => {
                let d = match opus::Decoder::new(SAMPLE_RATE, channels) {
                    Ok(d) => d,
                    Err(_) => continue,
                };
                decoders.entry(ssrc).or_insert(d)
            }
        };
        let samples = match decoder.decode(&opus, &mut pcm, false) {
            Ok(s) => {
                dec_ok += 1;
                s
            }
            Err(_) => match decoder.decode(&[], &mut pcm, false) {
                Ok(s) => s,
                Err(_) => continue,
            },
        };
        let mut m = ring.lock().unwrap();
        let e = m.entry(ssrc).or_default();
        let cap = FRAME_SAMPLES * 10;
        if e.buf.len() > cap {
            let d = e.buf.len() - FRAME_SAMPLES * 4;
            e.buf.drain(..d);
        }
        for s in &pcm[..samples] {
            e.buf.push_back(*s as f32 / 32768.0);
        }
    }
}
