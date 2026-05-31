use aes::cipher::{BlockEncrypt, KeyInit as _, KeyIvInit, StreamCipher};
use aes::Aes128;
use aes_gcm::aead::{Aead, KeyInit as _, Payload};
use aes_gcm::{Aes128Gcm, Nonce};
use ctr::Ctr32BE;
use ghash::universal_hash::{KeyInit as _, UniversalHash};
use ghash::GHash;
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub const MAGIC: [u8; 2] = [0xFA, 0xFA];
const TAG_LEN: usize = 8;
const KEY_LEN: usize = 16;
const SECRET_LEN: usize = 32;

type HmacSha256 = Hmac<Sha256>;

fn hkdf_expand(prk: &[u8], info: &[u8], length: usize) -> Vec<u8> {
    let mut okm = Vec::with_capacity(length);
    let mut t: Vec<u8> = Vec::new();
    let mut counter: u8 = 1;
    while okm.len() < length {
        let mut mac = <HmacSha256 as Mac>::new_from_slice(prk).expect("hmac key");
        mac.update(&t);
        mac.update(info);
        mac.update(&[counter]);
        t = mac.finalize().into_bytes().to_vec();
        okm.extend_from_slice(&t);
        counter = counter.wrapping_add(1);
    }
    okm.truncate(length);
    okm
}

fn expand_with_label(secret: &[u8], label: &str, context: &[u8], length: usize) -> Vec<u8> {
    let full_label = format!("MLS 1.0 {label}");
    let mut info = Vec::new();
    info.extend_from_slice(&(length as u16).to_be_bytes());
    super::wire::mls_varint_encode(full_label.len(), &mut info);
    info.extend_from_slice(full_label.as_bytes());
    super::wire::mls_varint_encode(context.len(), &mut info);
    info.extend_from_slice(context);
    hkdf_expand(secret, &info, length)
}

#[derive(Clone)]
pub struct KeyRatchet {
    secret: Vec<u8>,
    generation: u32,
}

impl KeyRatchet {
    pub fn new(base_secret: Vec<u8>) -> Self {
        Self {
            secret: base_secret,
            generation: 0,
        }
    }

    fn ratchet_forward(&mut self) {
        let next = expand_with_label(
            &self.secret,
            "secret",
            &self.generation.to_be_bytes(),
            SECRET_LEN,
        );
        self.secret = next;
        self.generation += 1;
    }

    pub fn key(&mut self, generation: u32) -> Option<[u8; KEY_LEN]> {
        if generation < self.generation {
            return None;
        }
        while self.generation < generation {
            self.ratchet_forward();
        }
        let k = expand_with_label(&self.secret, "key", &self.generation.to_be_bytes(), KEY_LEN);
        let mut arr = [0u8; KEY_LEN];
        arr.copy_from_slice(&k);
        Some(arr)
    }
}

fn full_nonce(truncated: u32) -> [u8; 12] {
    let mut n = [0u8; 12];
    n[8..12].copy_from_slice(&truncated.to_le_bytes());
    n
}

pub fn encrypt_opus(ratchet: &mut KeyRatchet, nonce: u32, opus_frame: &[u8]) -> Option<Vec<u8>> {
    let generation = nonce >> 24;
    let key = ratchet.key(generation)?;
    let cipher = Aes128Gcm::new_from_slice(&key).ok()?;
    let full = full_nonce(nonce);

    let ct_and_tag = cipher
        .encrypt(
            Nonce::from_slice(&full),
            Payload {
                msg: opus_frame,
                aad: &[],
            },
        )
        .ok()?;
    let split = ct_and_tag.len() - 16;
    let (ct, full_tag) = ct_and_tag.split_at(split);
    let tag8 = &full_tag[..TAG_LEN];

    let mut nonce_bytes = Vec::new();
    super::wire::uleb128_encode(nonce as u64, &mut nonce_bytes);

    let supp_size = TAG_LEN + nonce_bytes.len() + 1 + 2;
    let mut out = Vec::with_capacity(ct.len() + supp_size);
    out.extend_from_slice(ct);
    out.extend_from_slice(tag8);
    out.extend_from_slice(&nonce_bytes);
    out.push(supp_size as u8);
    out.extend_from_slice(&MAGIC);
    Some(out)
}

fn gcm_open_trunc8(key: &[u8; 16], nonce96: &[u8; 12], ct: &[u8], tag8: &[u8]) -> Option<Vec<u8>> {
    let aes = Aes128::new_from_slice(key).ok()?;

    let mut h = [0u8; 16];
    aes.encrypt_block((&mut h).into());

    let mut ghash = GHash::new_from_slice(&h).ok()?;
    ghash.update_padded(ct);
    let mut len_block = [0u8; 16];
    len_block[0..8].copy_from_slice(&0u64.to_be_bytes());
    len_block[8..16].copy_from_slice(&((ct.len() as u64) * 8).to_be_bytes());
    ghash.update(&[len_block.into()]);
    let s = ghash.finalize();

    let mut j0 = [0u8; 16];
    j0[..12].copy_from_slice(nonce96);
    j0[15] = 1;
    let mut ek_j0 = j0;
    aes.encrypt_block((&mut ek_j0).into());
    let mut full_tag = [0u8; 16];
    for i in 0..16 {
        full_tag[i] = ek_j0[i] ^ s[i];
    }

    let mut diff = 0u8;
    for i in 0..TAG_LEN {
        diff |= full_tag[i] ^ tag8[i];
    }
    if diff != 0 {
        return None;
    }

    let mut ctr_iv = [0u8; 16];
    ctr_iv[..12].copy_from_slice(nonce96);
    ctr_iv[15] = 2;
    let mut cipher = Ctr32BE::<Aes128>::new(key.into(), &ctr_iv.into());
    let mut pt = ct.to_vec();
    cipher.apply_keystream(&mut pt);
    Some(pt)
}

pub fn decrypt_opus(ratchet: &mut KeyRatchet, frame: &[u8]) -> Option<Vec<u8>> {
    if frame.len() < 3 || frame[frame.len() - 2..] != MAGIC {
        return None;
    }
    let supp_size = frame[frame.len() - 3] as usize;
    if supp_size < TAG_LEN + 1 + 1 + 2 || supp_size > frame.len() {
        return None;
    }
    let supp_start = frame.len() - supp_size;
    let supp = &frame[supp_start..];

    let tag8 = &supp[..TAG_LEN];
    let (nonce_val, _nlen) = super::wire::uleb128_decode(&supp[TAG_LEN..])?;
    let nonce = nonce_val as u32;
    let ciphertext = &frame[..supp_start];

    let generation = nonce >> 24;
    let key = ratchet.key(generation)?;
    gcm_open_trunc8(&key, &full_nonce(nonce), ciphertext, tag8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratchet_is_forward_only_and_deterministic() {
        let base = vec![7u8; 16];
        let mut r1 = KeyRatchet::new(base.clone());
        let mut r2 = KeyRatchet::new(base);
        assert_eq!(r1.key(3), r2.key(3));
        assert_eq!(r1.key(5), r2.key(5));
        assert_eq!(r1.key(2), None);
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let base = vec![1u8; 16];
        let mut enc = KeyRatchet::new(base.clone());
        let mut dec = KeyRatchet::new(base);
        let msg = b"the quick brown opus frame";
        let wire = encrypt_opus(&mut enc, 0x0000_002a, msg).unwrap();
        assert_eq!(wire[wire.len() - 2..], MAGIC);
        let back = decrypt_opus(&mut dec, &wire).expect("auth+decrypt");
        assert_eq!(back, msg);
    }

    #[test]
    fn tamper_is_rejected() {
        let base = vec![2u8; 16];
        let mut enc = KeyRatchet::new(base.clone());
        let mut dec = KeyRatchet::new(base);
        let mut wire = encrypt_opus(&mut enc, 1, b"hello").unwrap();
        wire[0] ^= 0x80;
        assert!(decrypt_opus(&mut dec, &wire).is_none());
    }
}
