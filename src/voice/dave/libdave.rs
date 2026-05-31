use std::ffi::{CString, c_char, c_void};
use std::os::raw::c_int;
use std::ptr;

#[repr(C)]
pub struct SessionS {
    _p: [u8; 0],
}
#[repr(C)]
pub struct CommitResultS {
    _p: [u8; 0],
}
#[repr(C)]
pub struct WelcomeResultS {
    _p: [u8; 0],
}
#[repr(C)]
pub struct KeyRatchetS {
    _p: [u8; 0],
}
#[repr(C)]
pub struct EncryptorS {
    _p: [u8; 0],
}
#[repr(C)]
pub struct DecryptorS {
    _p: [u8; 0],
}

pub type SessionHandle = *mut SessionS;
pub type CommitResultHandle = *mut CommitResultS;
pub type WelcomeResultHandle = *mut WelcomeResultS;
pub type KeyRatchetHandle = *mut KeyRatchetS;
pub type EncryptorHandle = *mut EncryptorS;
pub type DecryptorHandle = *mut DecryptorS;

pub const MEDIA_AUDIO: c_int = 0;
pub const MEDIA_VIDEO: c_int = 1;
pub const CODEC_OPUS: c_int = 1;
pub const CODEC_H264: c_int = 4;
pub const DEC_SUCCESS: c_int = 0;
pub const ENC_SUCCESS: c_int = 0;

type MlsFailureCb = extern "C" fn(*const c_char, *const c_char, *mut c_void);
type LogSinkCb = extern "C" fn(c_int, *const c_char, c_int, *const c_char);

extern "C" fn fwd_log_sink(sev: c_int, _file: *const c_char, _line: c_int, msg: *const c_char) {
    if msg.is_null() {
        return;
    }
    let m = unsafe { std::ffi::CStr::from_ptr(msg) }
        .to_string_lossy()
        .into_owned();
    if sev >= 3 {
        crate::error!("libdave: {m}");
    } else {
        crate::info!("libdave: {m}");
    }
}

pub fn silence_logs() {
    unsafe { daveSetLogSinkCallback(Some(fwd_log_sink)) }
}

unsafe extern "C" {
    pub fn daveFree(ptr: *mut c_void);
    pub fn daveMaxSupportedProtocolVersion() -> u16;

    pub fn daveSessionCreate(
        context: *mut c_void,
        auth_session_id: *const c_char,
        callback: Option<MlsFailureCb>,
        user_data: *mut c_void,
    ) -> SessionHandle;
    pub fn daveSessionDestroy(s: SessionHandle);
    pub fn daveSessionInit(
        s: SessionHandle,
        version: u16,
        group_id: u64,
        self_user_id: *const c_char,
    );
    pub fn daveSessionReset(s: SessionHandle);
    pub fn daveSessionSetExternalSender(
        s: SessionHandle,
        external_sender: *const u8,
        length: usize,
    );
    pub fn daveSessionProcessProposals(
        s: SessionHandle,
        proposals: *const u8,
        length: usize,
        recognized_user_ids: *const *const c_char,
        recognized_user_ids_length: usize,
        commit_welcome_bytes: *mut *mut u8,
        commit_welcome_bytes_length: *mut usize,
    );
    pub fn daveSessionProcessCommit(
        s: SessionHandle,
        commit: *const u8,
        length: usize,
    ) -> CommitResultHandle;
    pub fn daveSessionProcessWelcome(
        s: SessionHandle,
        welcome: *const u8,
        length: usize,
        recognized_user_ids: *const *const c_char,
        recognized_user_ids_length: usize,
    ) -> WelcomeResultHandle;
    pub fn daveSessionGetMarshalledKeyPackage(
        s: SessionHandle,
        key_package: *mut *mut u8,
        length: *mut usize,
    );
    pub fn daveSessionGetKeyRatchet(s: SessionHandle, user_id: *const c_char) -> KeyRatchetHandle;
    pub fn daveSessionGetLastEpochAuthenticator(
        s: SessionHandle,
        authenticator: *mut *mut u8,
        length: *mut usize,
    );

    pub fn daveKeyRatchetDestroy(kr: KeyRatchetHandle);

    pub fn daveCommitResultIsFailed(h: CommitResultHandle) -> bool;
    pub fn daveCommitResultIsIgnored(h: CommitResultHandle) -> bool;
    pub fn daveCommitResultDestroy(h: CommitResultHandle);
    pub fn daveWelcomeResultDestroy(h: WelcomeResultHandle);

    pub fn daveEncryptorCreate() -> EncryptorHandle;
    pub fn daveEncryptorDestroy(e: EncryptorHandle);
    pub fn daveEncryptorSetKeyRatchet(e: EncryptorHandle, kr: KeyRatchetHandle);
    pub fn daveEncryptorAssignSsrcToCodec(e: EncryptorHandle, ssrc: u32, codec: c_int);
    pub fn daveEncryptorSetPassthroughMode(e: EncryptorHandle, passthrough: bool);
    pub fn daveEncryptorGetMaxCiphertextByteSize(
        e: EncryptorHandle,
        media_type: c_int,
        frame_size: usize,
    ) -> usize;
    pub fn daveEncryptorEncrypt(
        e: EncryptorHandle,
        media_type: c_int,
        ssrc: u32,
        frame: *const u8,
        frame_length: usize,
        encrypted_frame: *mut u8,
        encrypted_frame_capacity: usize,
        bytes_written: *mut usize,
    ) -> c_int;

    pub fn daveDecryptorCreate() -> DecryptorHandle;
    pub fn daveDecryptorDestroy(d: DecryptorHandle);
    pub fn daveDecryptorTransitionToKeyRatchet(d: DecryptorHandle, kr: KeyRatchetHandle);
    pub fn daveDecryptorTransitionToPassthroughMode(d: DecryptorHandle, passthrough: bool);
    pub fn daveSetLogSinkCallback(cb: Option<LogSinkCb>);

    pub fn daveDecryptorGetMaxPlaintextByteSize(
        d: DecryptorHandle,
        media_type: c_int,
        encrypted_frame_size: usize,
    ) -> usize;
    pub fn daveDecryptorDecrypt(
        d: DecryptorHandle,
        media_type: c_int,
        encrypted_frame: *const u8,
        encrypted_frame_length: usize,
        frame: *mut u8,
        frame_capacity: usize,
        bytes_written: *mut usize,
    ) -> c_int;
}

pub struct Session {
    pub h: SessionHandle,
}

unsafe impl Send for Session {}

impl Session {
    pub fn create() -> Option<Self> {
        unsafe {
            let h = daveSessionCreate(ptr::null_mut(), ptr::null(), None, ptr::null_mut());
            if h.is_null() { None } else { Some(Self { h }) }
        }
    }

    pub fn set_external_sender(&self, body: &[u8]) {
        unsafe { daveSessionSetExternalSender(self.h, body.as_ptr(), body.len()) }
    }

    pub fn init(&self, self_user_id: u64, group_id: u64, version: u16) -> bool {
        match CString::new(self_user_id.to_string()) {
            Ok(uid) => {
                unsafe { daveSessionInit(self.h, version, group_id, uid.as_ptr()) };
                true
            }
            Err(_) => false,
        }
    }

    pub fn key_package(&self) -> Option<Vec<u8>> {
        unsafe {
            let mut p: *mut u8 = ptr::null_mut();
            let mut n: usize = 0;
            daveSessionGetMarshalledKeyPackage(self.h, &mut p, &mut n);
            if p.is_null() || n == 0 {
                return None;
            }
            let v = std::slice::from_raw_parts(p, n).to_vec();
            daveFree(p as *mut c_void);
            Some(v)
        }
    }

    pub fn process_proposals(&self, body: &[u8], recognized: &[u64]) -> Option<Vec<u8>> {
        unsafe {
            let cstrs: Vec<CString> = recognized
                .iter()
                .filter_map(|u| CString::new(u.to_string()).ok())
                .collect();
            let ptrs: Vec<*const c_char> = cstrs.iter().map(|c| c.as_ptr()).collect();
            let mut out: *mut u8 = ptr::null_mut();
            let mut outlen: usize = 0;
            daveSessionProcessProposals(
                self.h,
                body.as_ptr(),
                body.len(),
                ptrs.as_ptr(),
                ptrs.len(),
                &mut out,
                &mut outlen,
            );
            if out.is_null() || outlen == 0 {
                return None;
            }
            let v = std::slice::from_raw_parts(out, outlen).to_vec();
            daveFree(out as *mut c_void);
            Some(v)
        }
    }

    pub fn process_commit(&self, body: &[u8]) -> bool {
        unsafe {
            let h = daveSessionProcessCommit(self.h, body.as_ptr(), body.len());
            if h.is_null() {
                return false;
            }
            let failed = daveCommitResultIsFailed(h);
            daveCommitResultDestroy(h);
            !failed
        }
    }

    pub fn process_welcome(&self, body: &[u8], recognized: &[u64]) -> bool {
        unsafe {
            let cstrs: Vec<CString> = recognized
                .iter()
                .filter_map(|u| CString::new(u.to_string()).ok())
                .collect();
            let ptrs: Vec<*const c_char> = cstrs.iter().map(|c| c.as_ptr()).collect();
            let h = daveSessionProcessWelcome(
                self.h,
                body.as_ptr(),
                body.len(),
                ptrs.as_ptr(),
                ptrs.len(),
            );
            if h.is_null() {
                return false;
            }
            daveWelcomeResultDestroy(h);
            true
        }
    }

    pub fn key_ratchet(&self, user_id: u64) -> KeyRatchetHandle {
        unsafe {
            match CString::new(user_id.to_string()) {
                Ok(c) => daveSessionGetKeyRatchet(self.h, c.as_ptr()),
                Err(_) => ptr::null_mut(),
            }
        }
    }

    pub fn epoch_authenticator(&self) -> Option<Vec<u8>> {
        unsafe {
            let mut p: *mut u8 = ptr::null_mut();
            let mut n: usize = 0;
            daveSessionGetLastEpochAuthenticator(self.h, &mut p, &mut n);
            if p.is_null() || n == 0 {
                return None;
            }
            let v = std::slice::from_raw_parts(p, n).to_vec();
            daveFree(p as *mut c_void);
            Some(v)
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        unsafe { daveSessionDestroy(self.h) }
    }
}

pub struct Decryptor {
    h: DecryptorHandle,
}
unsafe impl Send for Decryptor {}
impl Decryptor {
    pub fn new() -> Option<Self> {
        let h = unsafe { daveDecryptorCreate() };
        if h.is_null() {
            None
        } else {
            unsafe { daveDecryptorTransitionToPassthroughMode(h, false) };
            Some(Self { h })
        }
    }
    pub fn set_ratchet(&self, kr: KeyRatchetHandle) {
        unsafe { daveDecryptorTransitionToKeyRatchet(self.h, kr) }
    }
    pub fn decrypt(&self, enc: &[u8], out: &mut [u8]) -> Result<usize, i32> {
        self.decrypt_media(enc, out, MEDIA_AUDIO)
    }
    pub fn decrypt_video(&self, enc: &[u8], out: &mut [u8]) -> Result<usize, i32> {
        self.decrypt_media(enc, out, MEDIA_VIDEO)
    }
    fn decrypt_media(&self, enc: &[u8], out: &mut [u8], media: c_int) -> Result<usize, i32> {
        let mut written: usize = 0;
        let rc = unsafe {
            daveDecryptorDecrypt(
                self.h,
                media,
                enc.as_ptr(),
                enc.len(),
                out.as_mut_ptr(),
                out.len(),
                &mut written,
            )
        };
        if rc == DEC_SUCCESS {
            Ok(written)
        } else {
            Err(rc)
        }
    }
    pub fn max_plaintext(&self, enc_size: usize) -> usize {
        unsafe { daveDecryptorGetMaxPlaintextByteSize(self.h, MEDIA_AUDIO, enc_size) }
    }
    pub fn max_plaintext_video(&self, enc_size: usize) -> usize {
        unsafe { daveDecryptorGetMaxPlaintextByteSize(self.h, MEDIA_VIDEO, enc_size) }
    }
}
impl Drop for Decryptor {
    fn drop(&mut self) {
        unsafe { daveDecryptorDestroy(self.h) }
    }
}

pub struct Encryptor {
    h: EncryptorHandle,
}
unsafe impl Send for Encryptor {}
impl Encryptor {
    pub fn new() -> Option<Self> {
        let h = unsafe { daveEncryptorCreate() };
        if h.is_null() {
            None
        } else {
            unsafe { daveEncryptorSetPassthroughMode(h, false) };
            Some(Self { h })
        }
    }
    pub fn set_ratchet(&self, kr: KeyRatchetHandle) {
        unsafe { daveEncryptorSetKeyRatchet(self.h, kr) }
    }
    pub fn assign_ssrc_opus(&self, ssrc: u32) {
        unsafe { daveEncryptorAssignSsrcToCodec(self.h, ssrc, CODEC_OPUS) }
    }
    pub fn assign_ssrc_h264(&self, ssrc: u32) {
        unsafe { daveEncryptorAssignSsrcToCodec(self.h, ssrc, CODEC_H264) }
    }
    pub fn max_ciphertext(&self, frame_size: usize) -> usize {
        unsafe { daveEncryptorGetMaxCiphertextByteSize(self.h, MEDIA_AUDIO, frame_size) }
    }
    pub fn max_ciphertext_video(&self, frame_size: usize) -> usize {
        unsafe { daveEncryptorGetMaxCiphertextByteSize(self.h, MEDIA_VIDEO, frame_size) }
    }
    pub fn encrypt_video(&self, ssrc: u32, frame: &[u8], out: &mut [u8]) -> Option<usize> {
        let mut written: usize = 0;
        let rc = unsafe {
            daveEncryptorEncrypt(
                self.h,
                MEDIA_VIDEO,
                ssrc,
                frame.as_ptr(),
                frame.len(),
                out.as_mut_ptr(),
                out.len(),
                &mut written,
            )
        };
        if rc == ENC_SUCCESS {
            Some(written)
        } else {
            None
        }
    }
    pub fn encrypt(&self, ssrc: u32, frame: &[u8], out: &mut [u8]) -> Option<usize> {
        let mut written: usize = 0;
        let rc = unsafe {
            daveEncryptorEncrypt(
                self.h,
                MEDIA_AUDIO,
                ssrc,
                frame.as_ptr(),
                frame.len(),
                out.as_mut_ptr(),
                out.len(),
                &mut written,
            )
        };
        if rc == ENC_SUCCESS {
            Some(written)
        } else {
            None
        }
    }
}
impl Drop for Encryptor {
    fn drop(&mut self) {
        unsafe { daveEncryptorDestroy(self.h) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_links_and_session_roundtrips() {
        let v = unsafe { daveMaxSupportedProtocolVersion() };
        assert!(v >= 1, "max protocol version = {v}");
        let s = Session::create().expect("session create");
        assert!(s.init(204972632863539201, 0x2000827e95f54614, 1), "init");
        let kp = s.key_package().expect("key package");
        assert!(kp.len() > 32, "key package len = {}", kp.len());
    }
}
