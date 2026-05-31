use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use image::DynamicImage;
use ratatui::layout::Rect;
use ratatui_image::Resize;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;

pub struct StreamView {
    job_tx: Sender<(DynamicImage, Rect)>,
    done_rx: Receiver<StatefulProtocol>,
    pub ready: Option<StatefulProtocol>,
    latest: Option<DynamicImage>,
    area: Option<Rect>,
    busy: bool,
}

impl StreamView {
    pub fn new(picker: &Picker) -> Self {
        let (job_tx, job_rx) = channel::<(DynamicImage, Rect)>();
        let (done_tx, done_rx) = channel::<StatefulProtocol>();
        let pk = *picker;
        thread::spawn(move || {
            while let Ok((img, area)) = job_rx.recv() {
                let mut p = pk.new_resize_protocol(img);
                p.resize_encode(&Resize::Fit(None), p.background_color(), area);
                if done_tx.send(p).is_err() {
                    break;
                }
            }
        });
        Self {
            job_tx,
            done_rx,
            ready: None,
            latest: None,
            area: None,
            busy: false,
        }
    }

    pub fn update_frame(&mut self, img: DynamicImage) {
        self.latest = Some(img);
        self.dispatch();
    }

    pub fn set_area(&mut self, area: Rect) {
        if self.area != Some(area) {
            self.area = Some(area);
            if self.ready.is_some() && self.latest.is_none() {
                self.latest = None;
            }
            self.dispatch();
        }
    }

    fn dispatch(&mut self) {
        if self.busy {
            return;
        }
        let (Some(area), Some(img)) = (self.area, self.latest.take()) else {
            return;
        };
        if area.width < 2 || area.height < 2 {
            self.latest = Some(img);
            return;
        }
        self.busy = true;
        let _ = self.job_tx.send((img, area));
    }

    pub fn poll(&mut self) {
        let mut got = false;
        while let Ok(p) = self.done_rx.try_recv() {
            self.ready = Some(p);
            got = true;
        }
        if got {
            self.busy = false;
            self.dispatch();
        }
    }
}
