use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

pub struct ScreenCapture {
    pub rx: Receiver<(u32, u32, Vec<u8>)>,
    running: Arc<AtomicBool>,
}

impl ScreenCapture {
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

impl Drop for ScreenCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn spawn_capture(fps: u32) -> Option<ScreenCapture> {
    #[cfg(target_os = "linux")]
    {
        if is_wayland() {
            let running = Arc::new(AtomicBool::new(true));
            let (tx, rx): (Sender<(u32, u32, Vec<u8>)>, _) = std::sync::mpsc::channel();
            if crate::voice::capture_wayland::spawn(fps, tx, running.clone()) {
                return Some(ScreenCapture { rx, running });
            }
            crate::error!("wayland capture unavailable, falling back to scrap");
        }
    }
    spawn_scrap(fps)
}

#[cfg(target_os = "linux")]
fn is_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|v| v.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
}

fn spawn_scrap(fps: u32) -> Option<ScreenCapture> {
    use scrap::{Capturer, Display};

    let display = Display::primary().ok()?;
    let running = Arc::new(AtomicBool::new(true));
    let (tx, rx): (Sender<(u32, u32, Vec<u8>)>, _) = std::sync::mpsc::channel();
    let run = running.clone();
    let frame_dt = Duration::from_millis((1000 / fps.max(1)) as u64);

    std::thread::spawn(move || {
        let mut cap = match Capturer::new(display) {
            Ok(c) => c,
            Err(e) => {
                crate::error!("screen capture init failed: {e}");
                return;
            }
        };
        let w = cap.width();
        let h = cap.height();
        crate::info!("screen capture: {w}x{h} @ {fps}fps");
        while run.load(Ordering::Relaxed) {
            let t0 = Instant::now();
            match cap.frame() {
                Ok(frame) => {
                    let stride = frame.len() / h.max(1);
                    let mut rgb = vec![0u8; w * h * 3];
                    for y in 0..h {
                        let row = &frame[y * stride..y * stride + w * 4];
                        let o = y * w * 3;
                        for x in 0..w {
                            let p = x * 4;
                            rgb[o + x * 3] = row[p + 2];
                            rgb[o + x * 3 + 1] = row[p + 1];
                            rgb[o + x * 3 + 2] = row[p];
                        }
                    }
                    if tx.send((w as u32, h as u32, rgb)).is_err() {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                }
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
            }
            let el = t0.elapsed();
            if el < frame_dt {
                std::thread::sleep(frame_dt - el);
            }
        }
    });

    Some(ScreenCapture { rx, running })
}
