use std::os::raw::{c_int, c_void};
use std::sync::{Arc, Mutex};

type OSStatus = i32;
type CFAllocatorRef = *const c_void;
type CFDictionaryRef = *const c_void;
type CMFormatDescriptionRef = *const c_void;
type CMBlockBufferRef = *const c_void;
type CMSampleBufferRef = *const c_void;
type CVImageBufferRef = *const c_void;
type CVPixelBufferRef = *const c_void;
type VTDecompressionSessionRef = *const c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct CMTime {
    value: i64,
    timescale: i32,
    flags: u32,
    epoch: i64,
}

#[repr(C)]
struct VTDecompressionOutputCallbackRecord {
    callback: extern "C" fn(
        *mut c_void,
        *mut c_void,
        OSStatus,
        u32,
        CVImageBufferRef,
        CMTime,
        CMTime,
    ),
    refcon: *mut c_void,
}

unsafe extern "C" {
    fn CFRelease(cf: *const c_void);
    fn CMVideoFormatDescriptionCreateFromH264ParameterSets(
        allocator: CFAllocatorRef,
        parameter_set_count: usize,
        parameter_set_pointers: *const *const u8,
        parameter_set_sizes: *const usize,
        nal_unit_header_length: c_int,
        format_description_out: *mut CMFormatDescriptionRef,
    ) -> OSStatus;
    fn VTDecompressionSessionCreate(
        allocator: CFAllocatorRef,
        video_format_description: CMFormatDescriptionRef,
        video_decoder_specification: CFDictionaryRef,
        destination_image_buffer_attributes: CFDictionaryRef,
        output_callback: *const VTDecompressionOutputCallbackRecord,
        session_out: *mut VTDecompressionSessionRef,
    ) -> OSStatus;
    fn VTDecompressionSessionDecodeFrame(
        session: VTDecompressionSessionRef,
        sample_buffer: CMSampleBufferRef,
        decode_flags: u32,
        source_frame_refcon: *mut c_void,
        info_flags_out: *mut u32,
    ) -> OSStatus;
    fn VTDecompressionSessionWaitForAsynchronousFrames(
        session: VTDecompressionSessionRef,
    ) -> OSStatus;
    fn VTDecompressionSessionInvalidate(session: VTDecompressionSessionRef);
    fn CMBlockBufferCreateWithMemoryBlock(
        allocator: CFAllocatorRef,
        memory_block: *mut c_void,
        block_length: usize,
        block_allocator: CFAllocatorRef,
        custom_block_source: *const c_void,
        offset_to_data: usize,
        data_length: usize,
        flags: u32,
        block_buffer_out: *mut CMBlockBufferRef,
    ) -> OSStatus;
    fn CMBlockBufferReplaceDataBytes(
        source_bytes: *const c_void,
        destination_buffer: CMBlockBufferRef,
        offset_into_destination: usize,
        data_length: usize,
    ) -> OSStatus;
    fn CMSampleBufferCreateReady(
        allocator: CFAllocatorRef,
        data_buffer: CMBlockBufferRef,
        format_description: CMFormatDescriptionRef,
        num_samples: isize,
        num_sample_timing_entries: isize,
        sample_timing_array: *const c_void,
        num_sample_size_entries: isize,
        sample_size_array: *const usize,
        sample_buffer_out: *mut CMSampleBufferRef,
    ) -> OSStatus;
    fn CVPixelBufferLockBaseAddress(pb: CVPixelBufferRef, flags: u64) -> i32;
    fn CVPixelBufferUnlockBaseAddress(pb: CVPixelBufferRef, flags: u64) -> i32;
    fn CVPixelBufferGetWidth(pb: CVPixelBufferRef) -> usize;
    fn CVPixelBufferGetHeight(pb: CVPixelBufferRef) -> usize;
    fn CVPixelBufferGetPixelFormatType(pb: CVPixelBufferRef) -> u32;
    fn CVPixelBufferGetBaseAddressOfPlane(
        pb: CVPixelBufferRef,
        plane: usize,
    ) -> *mut c_void;
    fn CVPixelBufferGetBytesPerRowOfPlane(
        pb: CVPixelBufferRef,
        plane: usize,
    ) -> usize;
}

const FMT_420V: u32 = 0x34323076; // '420v'
const FMT_420F: u32 = 0x34323066; // '420f'

type FrameSlot = Arc<Mutex<Option<(u32, u32, Vec<u8>)>>>;

fn nv12_to_rgb(
    y: *const u8,
    ys: usize,
    uv: *const u8,
    uvs: usize,
    w: usize,
    h: usize,
) -> Vec<u8> {
    let mut out = vec![0u8; w * h * 3];
    for j in 0..h {
        let yrow = unsafe { y.add(j * ys) };
        let uvrow = unsafe { uv.add((j / 2) * uvs) };
        for i in 0..w {
            let yy = unsafe { *yrow.add(i) } as f32 - 16.0;
            let cb = unsafe { *uvrow.add((i / 2) * 2) } as f32 - 128.0;
            let cr = unsafe { *uvrow.add((i / 2) * 2 + 1) } as f32 - 128.0;
            let r = 1.164 * yy + 1.596 * cr;
            let g = 1.164 * yy - 0.391 * cb - 0.813 * cr;
            let b = 1.164 * yy + 2.018 * cb;
            let o = (j * w + i) * 3;
            out[o] = r.clamp(0.0, 255.0) as u8;
            out[o + 1] = g.clamp(0.0, 255.0) as u8;
            out[o + 2] = b.clamp(0.0, 255.0) as u8;
        }
    }
    out
}

extern "C" fn output_cb(
    refcon: *mut c_void,
    _src: *mut c_void,
    status: OSStatus,
    _info: u32,
    image: CVImageBufferRef,
    _pts: CMTime,
    _dur: CMTime,
) {
    if status != 0 || image.is_null() {
        return;
    }
    let slot = unsafe { &*(refcon as *const Mutex<Option<(u32, u32, Vec<u8>)>>) };
    let pb = image as CVPixelBufferRef;
    unsafe {
        let fmt = CVPixelBufferGetPixelFormatType(pb);
        if fmt != FMT_420V && fmt != FMT_420F {
            return;
        }
        if CVPixelBufferLockBaseAddress(pb, 1) != 0 {
            return;
        }
        let w = CVPixelBufferGetWidth(pb);
        let h = CVPixelBufferGetHeight(pb);
        let yp = CVPixelBufferGetBaseAddressOfPlane(pb, 0) as *const u8;
        let ys = CVPixelBufferGetBytesPerRowOfPlane(pb, 0);
        let uvp = CVPixelBufferGetBaseAddressOfPlane(pb, 1) as *const u8;
        let uvs = CVPixelBufferGetBytesPerRowOfPlane(pb, 1);
        let rgb = if yp.is_null() || uvp.is_null() {
            Vec::new()
        } else {
            nv12_to_rgb(yp, ys, uvp, uvs, w, h)
        };
        CVPixelBufferUnlockBaseAddress(pb, 1);
        if !rgb.is_empty() {
            crate::voice::video::note_decoded();
            *slot.lock().unwrap() = Some((w as u32, h as u32, rgb));
        }
    }
}

fn nal_units(au: &[u8]) -> Vec<&[u8]> {
    let mut nals = Vec::new();
    let mut i = 0;
    let mut start = None;
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

pub struct VtDecoder {
    session: VTDecompressionSessionRef,
    fmt: CMFormatDescriptionRef,
    sps: Vec<u8>,
    pps: Vec<u8>,
    slot: FrameSlot,
    cb: Box<VTDecompressionOutputCallbackRecord>,
}

unsafe impl Send for VtDecoder {}

impl VtDecoder {
    pub fn new() -> Option<Self> {
        let slot: FrameSlot = Arc::new(Mutex::new(None));
        Some(Self {
            session: std::ptr::null(),
            fmt: std::ptr::null(),
            sps: Vec::new(),
            pps: Vec::new(),
            slot,
            cb: Box::new(VTDecompressionOutputCallbackRecord {
                callback: output_cb,
                refcon: std::ptr::null_mut(),
            }),
        })
    }

    fn ensure_session(&mut self, sps: &[u8], pps: &[u8]) -> bool {
        if !self.session.is_null() && self.sps == sps && self.pps == pps {
            return true;
        }
        self.teardown();
        self.sps = sps.to_vec();
        self.pps = pps.to_vec();
        let ptrs: [*const u8; 2] = [self.sps.as_ptr(), self.pps.as_ptr()];
        let sizes: [usize; 2] = [self.sps.len(), self.pps.len()];
        let mut fmt: CMFormatDescriptionRef = std::ptr::null();
        let st = unsafe {
            CMVideoFormatDescriptionCreateFromH264ParameterSets(
                std::ptr::null(),
                2,
                ptrs.as_ptr(),
                sizes.as_ptr(),
                4,
                &mut fmt,
            )
        };
        if st != 0 || fmt.is_null() {
            return false;
        }
        self.fmt = fmt;
        self.cb.refcon =
            Arc::as_ptr(&self.slot) as *const c_void as *mut c_void;
        let mut sess: VTDecompressionSessionRef = std::ptr::null();
        let st = unsafe {
            VTDecompressionSessionCreate(
                std::ptr::null(),
                self.fmt,
                std::ptr::null(),
                std::ptr::null(),
                &*self.cb,
                &mut sess,
            )
        };
        if st != 0 || sess.is_null() {
            return false;
        }
        self.session = sess;
        true
    }

    fn teardown(&mut self) {
        unsafe {
            if !self.session.is_null() {
                VTDecompressionSessionInvalidate(self.session);
                CFRelease(self.session);
                self.session = std::ptr::null();
            }
            if !self.fmt.is_null() {
                CFRelease(self.fmt);
                self.fmt = std::ptr::null();
            }
        }
    }

    pub fn take_latest(&self) -> Option<(u32, u32, Vec<u8>)> {
        if self.session.is_null() {
            return None;
        }
        unsafe {
            VTDecompressionSessionWaitForAsynchronousFrames(self.session);
        }
        self.slot.lock().unwrap().take()
    }

    pub fn feed(&mut self, au: &[u8]) -> bool {
        let nals = nal_units(au);
        let mut sps: Option<&[u8]> = None;
        let mut pps: Option<&[u8]> = None;
        for n in &nals {
            match n.first().map(|b| b & 0x1F) {
                Some(7) => sps = Some(n),
                Some(8) => pps = Some(n),
                _ => {}
            }
        }
        if let (Some(s), Some(p)) = (sps, pps) {
            self.ensure_session(s, p);
        }
        if self.session.is_null() {
            return false;
        }
        let mut avcc: Vec<u8> = Vec::with_capacity(au.len() + 16);
        for n in &nals {
            let t = n.first().map(|b| b & 0x1F).unwrap_or(0);
            if t == 7 || t == 8 {
                continue;
            }
            avcc.extend_from_slice(&(n.len() as u32).to_be_bytes());
            avcc.extend_from_slice(n);
        }
        if avcc.is_empty() {
            return false;
        }
        unsafe {
            let mut bb: CMBlockBufferRef = std::ptr::null();
            if CMBlockBufferCreateWithMemoryBlock(
                std::ptr::null(),
                std::ptr::null_mut(),
                avcc.len(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                avcc.len(),
                0,
                &mut bb,
            ) != 0
                || bb.is_null()
            {
                return false;
            }
            if CMBlockBufferReplaceDataBytes(
                avcc.as_ptr() as *const c_void,
                bb,
                0,
                avcc.len(),
            ) != 0
            {
                CFRelease(bb);
                return false;
            }
            let sizes = [avcc.len()];
            let mut sb: CMSampleBufferRef = std::ptr::null();
            if CMSampleBufferCreateReady(
                std::ptr::null(),
                bb,
                self.fmt,
                1,
                0,
                std::ptr::null(),
                1,
                sizes.as_ptr(),
                &mut sb,
            ) != 0
                || sb.is_null()
            {
                CFRelease(bb);
                return false;
            }
            VTDecompressionSessionDecodeFrame(
                self.session,
                sb,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            CFRelease(sb);
            CFRelease(bb);
        }
        true
    }
}

impl Drop for VtDecoder {
    fn drop(&mut self) {
        self.teardown();
    }
}
