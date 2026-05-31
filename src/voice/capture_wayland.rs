use std::os::fd::AsRawFd;
use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

type Frame = (u32, u32, Vec<u8>);

pub fn spawn(fps: u32, tx: Sender<Frame>, running: Arc<AtomicBool>) -> bool {
    if spawn_wlr(fps, tx.clone(), running.clone()) {
        return true;
    }
    crate::info!("wlr screencopy backend unavailable, trying xdg-desktop-portal");
    if spawn_portal(fps, tx, running) {
        return true;
    }
    crate::error!("no wayland capture backend could be started");
    false
}

fn spawn_wlr(fps: u32, tx: Sender<Frame>, running: Arc<AtomicBool>) -> bool {
    let connection = match libwayshot::WayshotConnection::new() {
        Ok(c) => c,
        Err(e) => {
            crate::info!("libwayshot connection failed: {e}");
            return false;
        }
    };

    if connection.get_all_outputs().is_empty() {
        crate::info!("libwayshot found no outputs");
        return false;
    }

    let fps = fps.max(1);
    let frame_dt = Duration::from_micros(1_000_000u64 / fps as u64);

    std::thread::spawn(move || {
        crate::info!("wlr screencopy capture started @ {fps}fps");
        let mut announced = false;
        while running.load(Ordering::Relaxed) {
            let t0 = Instant::now();
            let output = {
                let outputs = connection.get_all_outputs();
                match outputs.first() {
                    Some(o) => o.clone(),
                    None => {
                        crate::error!("wlr capture: outputs disappeared");
                        break;
                    }
                }
            };

            match connection.screenshot_single_output(&output, false) {
                Ok(image) => {
                    let rgb = image.to_rgb8();
                    let w = rgb.width();
                    let h = rgb.height();
                    if !announced {
                        crate::info!("wlr capture: {w}x{h}");
                        announced = true;
                    }
                    let buf = rgb.into_raw();
                    if tx.send((w, h, buf)).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    crate::error!("wlr screenshot failed: {e}");
                    break;
                }
            }

            let elapsed = t0.elapsed();
            if elapsed < frame_dt {
                std::thread::sleep(frame_dt - elapsed);
            }
        }
        crate::info!("wlr screencopy capture stopped");
    });

    true
}

fn spawn_portal(fps: u32, tx: Sender<Frame>, running: Arc<AtomicBool>) -> bool {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Option<(OwnedFd, u32)>>();

    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                crate::error!("portal: failed to build tokio runtime: {e}");
                let _ = ready_tx.send(None);
                return;
            }
        };

        let negotiated = runtime.block_on(portal_negotiate());
        let (fd, node_id) = match negotiated {
            Some(v) => v,
            None => {
                let _ = ready_tx.send(None);
                return;
            }
        };

        if ready_tx.send(Some((fd, node_id))).is_err() {
            return;
        }
    });

    let (fd, node_id) = match ready_rx.recv() {
        Ok(Some(v)) => v,
        _ => return false,
    };

    std::thread::spawn(move || {
        if let Err(e) = pipewire_loop(fd, node_id, fps, tx, running) {
            crate::error!("portal pipewire loop failed: {e}");
        }
    });

    true
}

async fn portal_negotiate() -> Option<(OwnedFd, u32)> {
    use ashpd::desktop::PersistMode;
    use ashpd::desktop::screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType};

    let proxy = match Screencast::new().await {
        Ok(p) => p,
        Err(e) => {
            crate::error!("portal: Screencast proxy failed: {e}");
            return None;
        }
    };

    let session = match proxy.create_session(Default::default()).await {
        Ok(s) => s,
        Err(e) => {
            crate::error!("portal: create_session failed: {e}");
            return None;
        }
    };

    if let Err(e) = proxy
        .select_sources(
            &session,
            SelectSourcesOptions::default()
                .set_cursor_mode(CursorMode::Embedded)
                .set_sources(SourceType::Monitor)
                .set_multiple(false)
                .set_persist_mode(PersistMode::DoNot),
        )
        .await
    {
        crate::error!("portal: select_sources failed: {e}");
        return None;
    }

    let response = match proxy.start(&session, None, Default::default()).await {
        Ok(req) => match req.response() {
            Ok(r) => r,
            Err(e) => {
                crate::error!("portal: start response failed: {e}");
                return None;
            }
        },
        Err(e) => {
            crate::error!("portal: start failed: {e}");
            return None;
        }
    };

    let node_id = match response.streams().first() {
        Some(stream) => stream.pipe_wire_node_id(),
        None => {
            crate::error!("portal: no streams returned");
            return None;
        }
    };

    let fd = match proxy
        .open_pipe_wire_remote(&session, Default::default())
        .await
    {
        Ok(fd) => fd,
        Err(e) => {
            crate::error!("portal: open_pipe_wire_remote failed: {e}");
            return None;
        }
    };

    crate::info!("portal: negotiated pipewire node {node_id}");
    Some((fd, node_id))
}

struct PortalUserData {
    width: u32,
    height: u32,
    format: u32,
    tx: Sender<Frame>,
    running: Arc<AtomicBool>,
}

const FORMAT_BGRA: u32 = 0;
const FORMAT_BGRX: u32 = 1;
const FORMAT_RGBA: u32 = 2;
const FORMAT_RGBX: u32 = 3;

fn pipewire_loop(
    fd: OwnedFd,
    node_id: u32,
    fps: u32,
    tx: Sender<Frame>,
    running: Arc<AtomicBool>,
) -> Result<(), pipewire::Error> {
    use pipewire::spa;
    use pipewire::spa::pod::Pod;
    use spa::param::format::{MediaSubtype, MediaType};
    use spa::param::format_utils;
    use spa::param::video::VideoFormat;
    use spa::utils::Direction;

    pipewire::init();

    let mainloop = pipewire::main_loop::MainLoop::new(None)?;
    let context = pipewire::context::Context::new(&mainloop)?;
    let core = context.connect_fd(fd.as_raw_fd(), None)?;

    let data = PortalUserData {
        width: 0,
        height: 0,
        format: FORMAT_BGRX,
        tx,
        running: running.clone(),
    };

    let stream = pipewire::stream::Stream::new(
        &core,
        "discord-screen-capture",
        pipewire::properties::properties! {
            *pipewire::keys::MEDIA_TYPE => "Video",
            *pipewire::keys::MEDIA_CATEGORY => "Capture",
            *pipewire::keys::MEDIA_ROLE => "Camera",
        },
    )?;

    let loop_quit = mainloop.clone();

    let _listener = stream
        .add_local_listener_with_user_data(data)
        .param_changed(|_, user_data, id, param| {
            let Some(param) = param else {
                return;
            };
            if id != spa::param::ParamType::Format.as_raw() {
                return;
            }
            let (media_type, media_subtype) = match format_utils::parse_format(param) {
                Ok(v) => v,
                Err(_) => return,
            };
            if media_type != MediaType::Video || media_subtype != MediaSubtype::Raw {
                return;
            }
            let mut info = spa::param::video::VideoInfoRaw::new();
            if info.parse(param).is_err() {
                return;
            }
            let size = info.size();
            user_data.width = size.width;
            user_data.height = size.height;
            user_data.format = match info.format() {
                VideoFormat::BGRA => FORMAT_BGRA,
                VideoFormat::BGRx => FORMAT_BGRX,
                VideoFormat::RGBA => FORMAT_RGBA,
                VideoFormat::RGBx => FORMAT_RGBX,
                _ => FORMAT_BGRX,
            };
            crate::info!(
                "portal pipewire: {}x{} fmt {}",
                user_data.width,
                user_data.height,
                user_data.format
            );
        })
        .process(move |stream, user_data| {
            if !user_data.running.load(Ordering::Relaxed) {
                loop_quit.quit();
                return;
            }
            let mut buffer = match stream.dequeue_buffer() {
                Some(b) => b,
                None => return,
            };
            let datas = buffer.datas_mut();
            if datas.is_empty() {
                return;
            }
            let chunk_size = datas[0].chunk().size() as usize;
            let chunk_stride = datas[0].chunk().stride().max(0) as usize;
            let w = user_data.width as usize;
            let h = user_data.height as usize;
            if w == 0 || h == 0 {
                return;
            }
            let pixels = match datas[0].data() {
                Some(p) => p,
                None => return,
            };

            let stride = if chunk_stride >= w * 4 {
                chunk_stride
            } else if chunk_size >= w * h * 4 {
                w * 4
            } else {
                return;
            };
            if pixels.len() < stride * h {
                return;
            }

            let mut rgb = vec![0u8; w * h * 3];
            let fmt = user_data.format;
            for y in 0..h {
                let row = &pixels[y * stride..y * stride + w * 4];
                let o = y * w * 3;
                for x in 0..w {
                    let p = x * 4;
                    let (r, g, b) = match fmt {
                        FORMAT_RGBA | FORMAT_RGBX => (row[p], row[p + 1], row[p + 2]),
                        _ => (row[p + 2], row[p + 1], row[p]),
                    };
                    rgb[o + x * 3] = r;
                    rgb[o + x * 3 + 1] = g;
                    rgb[o + x * 3 + 2] = b;
                }
            }

            if user_data
                .tx
                .send((user_data.width, user_data.height, rgb))
                .is_err()
            {
                loop_quit.quit();
            }
        })
        .register()?;

    let target_fps = fps.max(1);
    let obj = spa::pod::object!(
        spa::utils::SpaTypes::ObjectParamFormat,
        spa::param::ParamType::EnumFormat,
        spa::pod::property!(
            spa::param::format::FormatProperties::MediaType,
            Id,
            MediaType::Video
        ),
        spa::pod::property!(
            spa::param::format::FormatProperties::MediaSubtype,
            Id,
            MediaSubtype::Raw
        ),
        spa::pod::property!(
            spa::param::format::FormatProperties::VideoFormat,
            Choice,
            Enum,
            Id,
            VideoFormat::BGRx,
            VideoFormat::BGRx,
            VideoFormat::BGRA,
            VideoFormat::RGBx,
            VideoFormat::RGBA,
        ),
        spa::pod::property!(
            spa::param::format::FormatProperties::VideoSize,
            Choice,
            Range,
            Rectangle,
            spa::utils::Rectangle {
                width: 1280,
                height: 720
            },
            spa::utils::Rectangle {
                width: 1,
                height: 1
            },
            spa::utils::Rectangle {
                width: 8192,
                height: 8192
            }
        ),
        spa::pod::property!(
            spa::param::format::FormatProperties::VideoFramerate,
            Choice,
            Range,
            Fraction,
            spa::utils::Fraction {
                num: target_fps,
                denom: 1
            },
            spa::utils::Fraction { num: 0, denom: 1 },
            spa::utils::Fraction {
                num: 1000,
                denom: 1
            }
        ),
    );
    let values: Vec<u8> = spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(obj),
    )
    .map_err(|_| pipewire::Error::CreationFailed)?
    .0
    .into_inner();

    let mut params = [Pod::from_bytes(&values).ok_or(pipewire::Error::CreationFailed)?];

    stream.connect(
        Direction::Input,
        Some(node_id),
        pipewire::stream::StreamFlags::AUTOCONNECT | pipewire::stream::StreamFlags::MAP_BUFFERS,
        &mut params,
    )?;

    crate::info!("portal pipewire capture started @ {fps}fps");

    let poll_quit = mainloop.clone();
    let poll_running = running.clone();
    let timer = mainloop.loop_().add_timer(move |_| {
        if !poll_running.load(Ordering::Relaxed) {
            poll_quit.quit();
        }
    });
    let _ = timer.update_timer(
        Some(Duration::from_millis(200)),
        Some(Duration::from_millis(200)),
    );

    mainloop.run();
    crate::info!("portal pipewire capture stopped");
    Ok(())
}
