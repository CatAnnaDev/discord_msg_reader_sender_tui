use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use byteorder::{BigEndian, ByteOrder};
use tokio::net::UdpSocket;

use crate::voice::dave::runtime::SharedRt;

const MTU: usize = 1100;

fn aead_nonce(counter: u32) -> [u8; 12] {
    let mut n = [0u8; 12];
    BigEndian::write_u32(&mut n[0..4], counter);
    n
}

fn nal_units(au: &[u8]) -> Vec<&[u8]> {
    let mut nals = Vec::new();
    let mut i = 0;
    let mut start: Option<usize> = None;
    while i + 3 <= au.len() {
        let sc3 = au[i] == 0 && au[i + 1] == 0 && au[i + 2] == 1;
        let sc4 = i + 4 <= au.len()
            && au[i] == 0
            && au[i + 1] == 0
            && au[i + 2] == 0
            && au[i + 3] == 1;
        if sc3 || sc4 {
            let hdr = if sc4 { 4 } else { 3 };
            if let Some(s) = start {
                nals.push(&au[s..i]);
            }
            i += hdr;
            start = Some(i);
        } else {
            i += 1;
        }
    }
    if let Some(s) = start {
        if s < au.len() {
            nals.push(&au[s..]);
        }
    }
    nals
}

// Build the RTP payloads for one NAL (single or FU-A fragments).
fn packetize_nal(nal: &[u8], out: &mut Vec<Vec<u8>>) {
    if nal.is_empty() {
        return;
    }
    if nal.len() <= MTU {
        out.push(nal.to_vec());
        return;
    }
    let nri = nal[0] & 0x60;
    let typ = nal[0] & 0x1F;
    let body = &nal[1..];
    let chunk = MTU - 2;
    let n = body.len().div_ceil(chunk);
    for (i, part) in body.chunks(chunk).enumerate() {
        let mut p = Vec::with_capacity(part.len() + 2);
        p.push(nri | 28); // FU indicator (type 28 = FU-A)
        let mut fu = typ;
        if i == 0 {
            fu |= 0x80; // start
        }
        if i == n - 1 {
            fu |= 0x40; // end
        }
        p.push(fu);
        p.extend_from_slice(part);
        out.push(p);
    }
}

pub fn spawn_broadcast(
    socket: Arc<UdpSocket>,
    remote: SocketAddr,
    cipher: Aes256Gcm,
    vssrc: u32,
    dave: SharedRt,
    running: Arc<AtomicBool>,
    fps: u32,
) {
    let (au_tx, au_rx) = std::sync::mpsc::channel::<Vec<u8>>();

    // Capture + H264 encode on a dedicated blocking thread.
    let run_enc = running.clone();
    std::thread::spawn(move || {
        use openh264::encoder::{Encoder, EncoderConfig};
        use openh264::formats::{RgbSliceU8, YUVBuffer};
        use openh264::OpenH264API;

        let Some(cap) = crate::voice::capture::spawn_capture(fps) else {
            crate::error!("broadcast: screen capture unavailable");
            return;
        };
        let cfg = EncoderConfig::new()
            .set_bitrate_bps(4_000_000)
            .max_frame_rate(fps as f32)
            .enable_skip_frame(true);
        let mut enc = match Encoder::with_api_config(
            OpenH264API::from_source(),
            cfg,
        ) {
            Ok(e) => e,
            Err(e) => {
                crate::error!("broadcast: openh264 encoder init: {e}");
                return;
            }
        };
        crate::info!("broadcast: capture+encode thread up");
        while run_enc.load(Ordering::Relaxed) {
            let mut frame = match cap.rx.recv() {
                Ok(f) => f,
                Err(_) => break,
            };
            while let Ok(n) = cap.rx.try_recv() {
                frame = n;
            }
            let (w, h, rgb) = frame;
            let src = RgbSliceU8::new(&rgb, (w as usize, h as usize));
            let yuv = YUVBuffer::from_rgb8_source(src);
            match enc.encode(&yuv) {
                Ok(bs) => {
                    let au = bs.to_vec();
                    if !au.is_empty() && au_tx.send(au).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    crate::error!("broadcast: encode: {e}");
                }
            }
        }
        crate::info!("broadcast: capture+encode thread exiting");
    });

    // Encrypt + RTP packetize + UDP send on an async task.
    tokio::spawn(async move {
        let mut seq: u16 = rand::random();
        let mut ts: u32 = rand::random();
        let mut nonce_ctr: u32 = 0;
        let mut sent: u64 = 0;
        while running.load(Ordering::Relaxed) {
            let au = match au_rx.recv() {
                Ok(a) => a,
                Err(_) => break,
            };
            let mut au = au;
            while let Ok(n) = au_rx.try_recv() {
                au = n;
            }
            let enc_au = {
                let g = dave.lock().unwrap();
                if !g.enabled {
                    continue;
                }
                match g.encrypt_video(&au) {
                    Some(v) => v,
                    None => continue,
                }
            };
            let nals = nal_units(&enc_au);
            let mut payloads: Vec<Vec<u8>> = Vec::new();
            for n in &nals {
                packetize_nal(n, &mut payloads);
            }
            let last = payloads.len();
            for (i, pl) in payloads.iter().enumerate() {
                let marker = i + 1 == last;
                let mut header = [0u8; 12];
                header[0] = 0x80;
                header[1] = 105 | if marker { 0x80 } else { 0 };
                BigEndian::write_u16(&mut header[2..4], seq);
                BigEndian::write_u32(&mut header[4..8], ts);
                BigEndian::write_u32(&mut header[8..12], vssrc);
                let nonce = aead_nonce(nonce_ctr);
                let ct = match cipher.encrypt(
                    Nonce::from_slice(&nonce),
                    Payload {
                        msg: pl,
                        aad: &header,
                    },
                ) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let mut pkt =
                    Vec::with_capacity(12 + ct.len() + 4);
                pkt.extend_from_slice(&header);
                pkt.extend_from_slice(&ct);
                let mut tail = [0u8; 4];
                BigEndian::write_u32(&mut tail, nonce_ctr);
                pkt.extend_from_slice(&tail);
                let _ = socket.send_to(&pkt, remote).await;
                seq = seq.wrapping_add(1);
                nonce_ctr = nonce_ctr.wrapping_add(1);
            }
            ts = ts.wrapping_add(90_000 / fps.max(1));
            sent += 1;
            if sent <= 5 || sent % 150 == 0 {
                crate::info!(
                    "broadcast send #{sent}: au={}B enc={}B nals={} pkts={}",
                    au.len(),
                    enc_au.len(),
                    nals.len(),
                    last
                );
            }
        }
        crate::info!("broadcast: send task exiting");
    });
}
