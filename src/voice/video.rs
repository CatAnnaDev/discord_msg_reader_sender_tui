use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use openh264::decoder::Decoder;
use openh264::formats::YUVSource;

static RECV_AUS: AtomicU64 = AtomicU64::new(0);
static DECODED: AtomicU64 = AtomicU64::new(0);
static DROPPED: AtomicU64 = AtomicU64::new(0);
static QLEN: AtomicUsize = AtomicUsize::new(0);
static FPS_S: OnceLock<Mutex<(Instant, u64, f32)>> = OnceLock::new();

pub struct VidStats {
    pub recv: u64,
    pub decoded: u64,
    pub dropped: u64,
    pub qlen: usize,
    pub fps: f32,
}

pub fn note_recv_au() {
    RECV_AUS.fetch_add(1, Ordering::Relaxed);
}

pub fn note_decoded() {
    DECODED.fetch_add(1, Ordering::Relaxed);
}

pub fn stats() -> VidStats {
    let decoded = DECODED.load(Ordering::Relaxed);
    let m = FPS_S.get_or_init(|| Mutex::new((Instant::now(), 0, 0.0)));
    let mut g = m.lock().unwrap();
    let dt = g.0.elapsed().as_secs_f32();
    if dt >= 1.0 {
        g.2 = (decoded - g.1) as f32 / dt;
        g.0 = Instant::now();
        g.1 = decoded;
    }
    VidStats {
        recv: RECV_AUS.load(Ordering::Relaxed),
        decoded,
        dropped: DROPPED.load(Ordering::Relaxed),
        qlen: QLEN.load(Ordering::Relaxed),
        fps: g.2,
    }
}

pub struct FrameSlot {
    pub w: u32,
    pub h: u32,
    pub rgb: Vec<u8>,
    pub genr: u64,
}

#[derive(Default)]
pub struct VideoState {
    pub frames: HashMap<u64, FrameSlot>,
    pub genr: u64,
}

static STATE: OnceLock<Mutex<VideoState>> = OnceLock::new();
static BIG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_big(b: bool) {
    BIG.store(b, std::sync::atomic::Ordering::Relaxed);
}

#[allow(dead_code)]
fn max_width() -> u32 {
    if BIG.load(std::sync::atomic::Ordering::Relaxed) {
        1920
    } else {
        960
    }
}

pub fn state() -> &'static Mutex<VideoState> {
    STATE.get_or_init(|| Mutex::new(VideoState::default()))
}

pub fn submit(uid: u64, w: u32, h: u32, rgb: Vec<u8>) {
    let mut s = state().lock().unwrap();
    s.genr += 1;
    let g = s.genr;
    s.frames.insert(uid, FrameSlot { w, h, rgb, genr: g });
}

pub fn snapshot(uid: u64) -> Option<(u64, u32, u32, Vec<u8>)> {
    let s = state().lock().unwrap();
    s.frames
        .get(&uid)
        .map(|f| (f.genr, f.w, f.h, f.rgb.clone()))
}

const START: [u8; 4] = [0, 0, 0, 1];

fn depacketize_into(au: &mut Vec<u8>, p: &[u8]) {
    if p.is_empty() {
        return;
    }
    match p[0] & 0x1F {
        1..=23 => {
            au.extend_from_slice(&START);
            au.extend_from_slice(p);
        }
        24 => {
            let mut i = 1usize;
            while i + 2 <= p.len() {
                let sz = u16::from_be_bytes([p[i], p[i + 1]]) as usize;
                i += 2;
                if i + sz > p.len() {
                    break;
                }
                au.extend_from_slice(&START);
                au.extend_from_slice(&p[i..i + sz]);
                i += sz;
            }
        }
        28 => {
            if p.len() < 2 {
                return;
            }
            let start = p[1] & 0x80 != 0;
            let hdr = (p[0] & 0xE0) | (p[1] & 0x1F);
            if start {
                au.extend_from_slice(&START);
                au.push(hdr);
            }
            au.extend_from_slice(&p[2..]);
        }
        _ => {}
    }
}

pub struct VideoRx {
    cur_ts: Option<u32>,
    frags: Vec<(u16, Vec<u8>)>,
}

impl VideoRx {
    pub fn new() -> Option<Self> {
        Some(Self {
            cur_ts: None,
            frags: Vec::with_capacity(256),
        })
    }

    fn build_au(&self) -> Vec<u8> {
        let mut ord = self.frags.clone();
        if let Some((r, _)) = ord.first().map(|(s, _)| (*s, ())) {
            ord.sort_by_key(|(s, _)| s.wrapping_sub(r));
        }
        let mut au = Vec::with_capacity(64 * 1024);
        for (_, p) in &ord {
            depacketize_into(&mut au, p);
        }
        au
    }

    pub fn push(&mut self, seq: u16, ts: u32, marker: bool, payload: &[u8]) -> Option<Vec<u8>> {
        let boundary = self.cur_ts.is_some_and(|c| c != ts);
        let mut done = if boundary && !self.frags.is_empty() {
            Some(self.build_au())
        } else {
            None
        };
        if boundary {
            self.frags.clear();
        }
        self.cur_ts = Some(ts);
        self.frags.push((seq, payload.to_vec()));
        if marker && !self.frags.is_empty() {
            done = Some(self.build_au());
            self.frags.clear();
            self.cur_ts = None;
        }
        done
    }
}

fn decode_one(decoder: &mut Decoder, clear: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let starts_annexb = clear.len() >= 4
        && ((clear[0] == 0 && clear[1] == 0 && clear[2] == 0 && clear[3] == 1)
            || (clear[0] == 0 && clear[1] == 0 && clear[2] == 1));
    let buf: Vec<u8> = if starts_annexb {
        clear.to_vec()
    } else {
        let mut v = Vec::with_capacity(clear.len() + 4);
        v.extend_from_slice(&[0, 0, 0, 1]);
        v.extend_from_slice(clear);
        v
    };
    let yuv = match decoder.decode(&buf) {
        Ok(Some(y)) => y,
        _ => return None,
    };
    let (w, h) = yuv.dimensions();
    let mut rgb = vec![0u8; w * h * 3];
    yuv.write_rgb8(&mut rgb);
    Some((w as u32, h as u32, rgb))
}

#[allow(dead_code)]
fn downscale(w: u32, h: u32, rgb: Vec<u8>) -> Option<(u32, u32, Vec<u8>)> {
    let cap = max_width();
    match image::RgbImage::from_raw(w, h, rgb) {
        Some(img) if w > cap => {
            let nw = cap;
            let nh = (h * cap / w).max(1);
            let small =
                image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle);
            Some((nw, nh, small.into_raw()))
        }
        Some(img) => Some((w, h, img.into_raw())),
        None => None,
    }
}

fn is_keyframe(au: &[u8]) -> bool {
    let mut i = 0;
    while i + 4 < au.len() {
        let sc3 = au[i] == 0 && au[i + 1] == 0 && au[i + 2] == 1;
        let sc4 = au[i] == 0 && au[i + 1] == 0 && au[i + 2] == 0 && au[i + 3] == 1;
        if sc3 || sc4 {
            let h = if sc4 { i + 4 } else { i + 3 };
            if h < au.len() {
                let t = au[h] & 0x1F;
                if t == 5 || t == 7 {
                    return true;
                }
            }
            i = h;
        } else {
            i += 1;
        }
    }
    false
}

pub fn spawn_decoder() -> std::sync::mpsc::Sender<(u64, Vec<u8>)> {
    let (tx, rx) = std::sync::mpsc::channel::<(u64, Vec<u8>)>();
    std::thread::spawn(move || {
        let mut decoder = match Decoder::new() {
            Ok(d) => d,
            Err(_) => {
                crate::error!("voice: openh264 decoder init failed");
                return;
            }
        };
        let mut vt = crate::voice::vtdec::VtDecoder::new();
        let mut vt_ok = false;
        let mut vt_logged = false;
        let mut fail: u32 = 0;
        let mut q: std::collections::VecDeque<(u64, Vec<u8>)> = std::collections::VecDeque::new();
        while let Ok(item) = rx.recv() {
            q.push_back(item);
            while let Ok(next) = rx.try_recv() {
                q.push_back(next);
            }
            // Behind: skip whole GOPs (only at keyframe boundaries) to bound
            // latency without breaking H264 inter-frame references.
            if q.len() > 6 {
                let mut cut = 0usize;
                for (i, (_, c)) in q.iter().enumerate() {
                    if is_keyframe(c) {
                        cut = i;
                    }
                }
                for _ in 0..cut {
                    q.pop_front();
                }
                if cut > 0 {
                    DROPPED.fetch_add(cut as u64, Ordering::Relaxed);
                }
            }
            QLEN.store(q.len(), Ordering::Relaxed);
            // Pipeline the WHOLE backlog into the HW decoder without waiting
            // per-frame (refs stay valid, decode is async on the GPU/ASIC),
            // then collect once → minimal standing latency.
            let mut latest: Option<(u64, u32, u32, Vec<u8>)> = None;
            let mut last_uid: Option<u64> = None;
            let mut vt_fed = false;
            while let Some((uid, clear)) = q.pop_front() {
                let fed = vt.as_mut().map(|d| d.feed(&clear)).unwrap_or(false);
                if fed {
                    if !vt_ok {
                        vt_ok = true;
                        if !vt_logged {
                            vt_logged = true;
                            crate::info!("voice: VideoToolbox HW decode active");
                        }
                    }
                    vt_fed = true;
                    last_uid = Some(uid);
                    continue;
                }
                if !vt_ok && !vt_logged {
                    vt_logged = true;
                    crate::info!("voice: VideoToolbox unavailable → openh264 software decode");
                }
                match decode_one(&mut decoder, &clear) {
                    Some((w, h, rgb)) => {
                        fail = 0;
                        DECODED.fetch_add(1, Ordering::Relaxed);
                        latest = Some((uid, w, h, rgb));
                    }
                    None => {
                        fail += 1;
                        if fail >= 30 {
                            if let Ok(d) = Decoder::new() {
                                decoder = d;
                            }
                            fail = 0;
                        }
                    }
                }
            }
            if vt_fed {
                if let (Some(vd), Some(uid)) = (vt.as_ref(), last_uid) {
                    if let Some((w, h, rgb)) = vd.take_latest() {
                        latest = Some((uid, w, h, rgb));
                    }
                }
            }
            if let Some((uid, w, h, rgb)) = latest {
                submit(uid, w, h, rgb);
            }
            QLEN.store(q.len(), Ordering::Relaxed);
        }
    });
    tx
}
