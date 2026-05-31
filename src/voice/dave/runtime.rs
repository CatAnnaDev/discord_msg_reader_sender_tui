use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::libdave::{Decryptor, Encryptor, KeyRatchetHandle, Session, daveKeyRatchetDestroy};
use super::wire;

pub struct DaveRt {
    session: Option<Session>,
    enc: Option<Encryptor>,
    dec: HashMap<u32, Decryptor>,
    ratchets: HashMap<u64, KeyRatchetHandle>,
    ssrc_uid: HashMap<u32, u64>,
    our_uid: u64,
    our_ssrc: u32,
    ext_sender: Option<Vec<u8>>,
    group_id: u64,
    inited: bool,
    watch_uid: Option<u64>,
    broadcast: bool,
    pub enabled: bool,
}

pub type SharedRt = Arc<Mutex<DaveRt>>;

unsafe impl Send for DaveRt {}

fn group_id_from_proposals(body: &[u8]) -> Option<u64> {
    let mut o = 1usize;
    let (_vlen, n) = wire::mls_varint_decode(body.get(o..)?)?;
    o += n;
    o += 4;
    let (gid_len, n) = wire::mls_varint_decode(body.get(o..)?)?;
    o += n;
    let gid = body.get(o..o + gid_len as usize)?;
    if gid.len() == 8 {
        Some(u64::from_be_bytes(gid.try_into().ok()?))
    } else {
        None
    }
}

impl DaveRt {
    pub fn new(our_uid: u64, our_ssrc: u32) -> SharedRt {
        super::libdave::silence_logs();
        Arc::new(Mutex::new(Self {
            session: None,
            enc: None,
            dec: HashMap::new(),
            ratchets: HashMap::new(),
            ssrc_uid: HashMap::new(),
            our_uid,
            our_ssrc,
            ext_sender: None,
            group_id: 0,
            inited: false,
            watch_uid: None,
            broadcast: false,
            enabled: false,
        }))
    }

    pub fn rebootstrap(&mut self) -> Option<Vec<u8>> {
        let es = self.ext_sender.clone()?;
        for (_, h) in self.ratchets.drain() {
            unsafe { daveKeyRatchetDestroy(h) };
        }
        self.dec.clear();
        self.enc = None;
        self.enabled = false;
        let s = Session::create()?;
        s.set_external_sender(&es);
        if !s.init(self.our_uid, self.group_id, 1) {
            return None;
        }
        let kp = s.key_package();
        self.session = Some(s);
        self.inited = true;
        kp
    }

    pub fn on_external_sender(&mut self, body: &[u8], group_id: u64) -> Option<Vec<u8>> {
        self.ext_sender = Some(body.to_vec());
        self.group_id = group_id;
        if !self.inited {
            let s = Session::create()?;
            s.set_external_sender(body);
            if !s.init(self.our_uid, group_id, 1) {
                return None;
            }
            self.session = Some(s);
            self.inited = true;
        } else if let Some(s) = &self.session {
            s.set_external_sender(body);
        }
        self.session.as_ref().and_then(|s| s.key_package())
    }

    pub fn on_proposals_get_op28(&mut self, op27: &[u8], recognized: &[u64]) -> Option<Vec<u8>> {
        self.session
            .as_ref()
            .and_then(|s| s.process_proposals(op27, recognized))
    }

    pub fn on_commit(&mut self, op29: &[u8]) -> bool {
        self.session
            .as_ref()
            .map(|s| s.process_commit(op29))
            .unwrap_or(false)
    }

    pub fn on_welcome(&mut self, op30: &[u8], recognized: &[u64]) -> bool {
        self.session
            .as_ref()
            .map(|s| s.process_welcome(op30, recognized))
            .unwrap_or(false)
    }

    pub fn epoch_authenticator(&self) -> Option<Vec<u8>> {
        self.session.as_ref().and_then(|s| s.epoch_authenticator())
    }

    pub fn verification_code(&self) -> Option<String> {
        let a = self.epoch_authenticator()?;
        if a.len() < 30 {
            return None;
        }
        let mut out = String::new();
        for grp in 0..6 {
            let mut v: u64 = 0;
            for b in 0..5 {
                v = (v << 8) | a[grp * 5 + b] as u64;
            }
            v %= 100_000;
            out.push_str(&format!("{v:05} "));
        }
        Some(out.trim_end().to_string())
    }

    fn ratchet_for(&mut self, uid: u64) -> KeyRatchetHandle {
        if let Some(h) = self.ratchets.get(&uid) {
            return *h;
        }
        let h = self
            .session
            .as_ref()
            .map(|s| s.key_ratchet(uid))
            .unwrap_or(std::ptr::null_mut());
        if !h.is_null() {
            self.ratchets.insert(uid, h);
        }
        h
    }

    pub fn arm(&mut self, speakers: &HashMap<u32, u64>) {
        if self.session.is_none() {
            return;
        }
        for (_, h) in self.ratchets.drain() {
            unsafe { daveKeyRatchetDestroy(h) };
        }
        self.dec.clear();
        let our = self.our_uid;
        let kr = self.ratchet_for(our);
        if !kr.is_null() {
            if self.enc.is_none() {
                self.enc = Encryptor::new();
            }
            if let Some(e) = &self.enc {
                if self.broadcast {
                    e.assign_ssrc_h264(self.our_ssrc);
                } else {
                    e.assign_ssrc_opus(self.our_ssrc);
                }
                e.set_ratchet(kr);
            }
        }
        let pairs: Vec<(u32, u64)> = speakers
            .iter()
            .filter(|(_, u)| **u != our)
            .map(|(s, u)| (*s, *u))
            .collect();
        for (ssrc, uid) in pairs {
            self.ssrc_uid.insert(ssrc, uid);
            self.bind_decryptor(ssrc, uid);
        }
        self.enabled = true;
    }

    fn bind_decryptor(&mut self, ssrc: u32, uid: u64) {
        let kr = self.ratchet_for(uid);
        if kr.is_null() {
            return;
        }
        if !self.dec.contains_key(&ssrc) {
            match Decryptor::new() {
                Some(d) => {
                    self.dec.insert(ssrc, d);
                }
                None => return,
            }
        }
        if let Some(d) = self.dec.get(&ssrc) {
            d.set_ratchet(kr);
        }
    }

    pub fn debug_state(&self) -> String {
        let mut decs: Vec<u32> = self.dec.keys().copied().collect();
        decs.sort_unstable();
        let mut rats: Vec<u64> = self.ratchets.keys().copied().collect();
        rats.sort_unstable();
        let mut sp: Vec<(u32, u64)> = self.ssrc_uid.iter().map(|(s, u)| (*s, *u)).collect();
        sp.sort_unstable();
        format!(
            "inited={} enabled={} dec_ssrcs={decs:?} ratchet_uids={rats:?} ssrc_uid={sp:?}",
            self.inited, self.enabled
        )
    }

    pub fn has_decryptor(&self, ssrc: u32) -> bool {
        self.dec.contains_key(&ssrc)
    }

    pub fn note_ssrc_uid(&mut self, ssrc: u32, uid: u64) {
        if uid == self.our_uid {
            return;
        }
        self.ssrc_uid.insert(ssrc, uid);
        if self.enabled {
            self.bind_decryptor(ssrc, uid);
        }
    }

    pub fn decrypt(&mut self, ssrc: u32, dave_frame: &[u8]) -> Result<Vec<u8>, i32> {
        let d = match self.dec.get(&ssrc) {
            Some(d) => d,
            None => return Err(-1),
        };
        let cap = d.max_plaintext(dave_frame.len()).max(dave_frame.len()) + 64;
        let mut out = vec![0u8; cap];
        match d.decrypt(dave_frame, &mut out) {
            Ok(n) => {
                out.truncate(n);
                Ok(out)
            }
            Err(rc) => Err(rc),
        }
    }

    pub fn decrypt_video(&mut self, ssrc: u32, dave_frame: &[u8]) -> Result<Vec<u8>, i32> {
        let d = match self.dec.get(&ssrc) {
            Some(d) => d,
            None => return Err(-1),
        };
        let cap = d
            .max_plaintext_video(dave_frame.len())
            .max(dave_frame.len())
            + 64;
        let mut out = vec![0u8; cap];
        match d.decrypt_video(dave_frame, &mut out) {
            Ok(n) => {
                out.truncate(n);
                Ok(out)
            }
            Err(rc) => Err(rc),
        }
    }

    pub fn uid_for(&self, ssrc: u32) -> Option<u64> {
        self.ssrc_uid.get(&ssrc).copied()
    }

    pub fn set_watch_uid(&mut self, uid: u64) {
        self.watch_uid = Some(uid);
    }

    pub fn set_broadcast(&mut self) {
        self.broadcast = true;
    }

    pub fn encrypt_video(&self, frame: &[u8]) -> Option<Vec<u8>> {
        let e = self.enc.as_ref()?;
        let cap = e.max_ciphertext_video(frame.len()).max(frame.len()) + 64;
        let mut out = vec![0u8; cap];
        let n = e.encrypt_video(self.our_ssrc, frame, &mut out)?;
        out.truncate(n);
        Some(out)
    }

    pub fn video_uid(&mut self, ssrc: u32) -> Option<u64> {
        let uid = self.ssrc_uid.get(&ssrc).copied().or(self.watch_uid)?;
        self.ssrc_uid.entry(ssrc).or_insert(uid);
        if self.enabled && !self.dec.contains_key(&ssrc) {
            self.bind_decryptor(ssrc, uid);
        }
        Some(uid)
    }

    pub fn encrypt(&self, opus: &[u8]) -> Option<Vec<u8>> {
        let e = self.enc.as_ref()?;
        let cap = e.max_ciphertext(opus.len()).max(opus.len()) + 64;
        let mut out = vec![0u8; cap];
        let n = e.encrypt(self.our_ssrc, opus, &mut out)?;
        out.truncate(n);
        Some(out)
    }
}

impl Drop for DaveRt {
    fn drop(&mut self) {
        for (_, h) in self.ratchets.drain() {
            unsafe { daveKeyRatchetDestroy(h) };
        }
    }
}
