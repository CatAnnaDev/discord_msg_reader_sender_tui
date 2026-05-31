use std::error::Error;

use openmls::prelude::*;
use openmls_basic_credential::SignatureKeyPair;
use openmls_rust_crypto::OpenMlsRustCrypto;
use openmls_traits::types::SignatureScheme;

use super::wire;

type BoxErr = Box<dyn Error + Send + Sync>;

const CIPHERSUITE: Ciphersuite = Ciphersuite::MLS_128_DHKEMP256_AES128GCM_SHA256_P256;

pub struct ExternalSenderInfo {
    pub signature_key: Vec<u8>,
    pub credential_identity: Vec<u8>,
}

pub struct DaveMls {
    provider: OpenMlsRustCrypto,
    signer: SignatureKeyPair,
    credential_with_key: CredentialWithKey,
    pub external_sender: Option<ExternalSenderInfo>,
    pub group: Option<MlsGroup>,
}

impl DaveMls {
    pub fn new(user_id: u64) -> Result<Self, BoxErr> {
        let provider = OpenMlsRustCrypto::default();
        let signer = SignatureKeyPair::new(SignatureScheme::ECDSA_SECP256R1_SHA256)
            .map_err(|e| format!("sig keypair: {e:?}"))?;
        signer
            .store(provider.storage())
            .map_err(|e| format!("store signer: {e:?}"))?;

        let credential = BasicCredential::new(user_id.to_be_bytes().to_vec());
        let credential_with_key = CredentialWithKey {
            credential: credential.into(),
            signature_key: signer.public().into(),
        };

        Ok(Self {
            provider,
            signer,
            credential_with_key,
            external_sender: None,
            group: None,
        })
    }

    fn dave_capabilities() -> Capabilities {
        Capabilities::new(
            Some(&[ProtocolVersion::Mls10]),
            Some(&[CIPHERSUITE]),
            Some(&[]),
            Some(&[]),
            Some(&[CredentialType::Basic]),
        )
    }

    pub fn key_package_op26(&self) -> Result<Vec<u8>, BoxErr> {
        let bundle = KeyPackage::builder()
            .key_package_lifetime(Lifetime::init(0, u64::MAX))
            .leaf_node_capabilities(Self::dave_capabilities())
            .build(
                CIPHERSUITE,
                &self.provider,
                &self.signer,
                self.credential_with_key.clone(),
            )
            .map_err(|e| format!("key package build: {e:?}"))?;

        use tls_codec::Serialize as _;
        let kp: KeyPackage = bundle.key_package().clone();
        let bytes = kp
            .tls_serialize_detached()
            .map_err(|e| format!("kp serialize: {e:?}"))?;
        Ok(wire::frame_outbound(wire::op::MLS_KEY_PACKAGE, &bytes))
    }

    pub fn set_external_sender(&mut self, body: &[u8]) -> Result<(), BoxErr> {
        let mut o = 0usize;

        let (sig_len, n) = wire::mls_varint_decode(&body[o..]).ok_or("ext sender: bad sig len")?;
        o += n;
        let sig_len = sig_len as usize;
        if o + sig_len > body.len() {
            return Err("ext sender: sig overflow".into());
        }
        let signature_key = body[o..o + sig_len].to_vec();
        o += sig_len;

        if o + 2 > body.len() {
            return Err("ext sender: missing credential type".into());
        }
        let cred_type = u16::from_be_bytes([body[o], body[o + 1]]);
        o += 2;
        if cred_type != 1 {
            return Err(format!("ext sender: unexpected credential type {cred_type}").into());
        }
        let (id_len, n) = wire::mls_varint_decode(&body[o..]).ok_or("ext sender: bad id len")?;
        o += n;
        let id_len = id_len as usize;
        if o + id_len > body.len() {
            return Err("ext sender: identity overflow".into());
        }
        let credential_identity = body[o..o + id_len].to_vec();

        self.external_sender = Some(ExternalSenderInfo {
            signature_key,
            credential_identity,
        });
        Ok(())
    }

    pub fn create_local_group(&mut self, group_id: &[u8]) -> Result<(), BoxErr> {
        let es = self
            .external_sender
            .as_ref()
            .ok_or("no external sender yet")?;

        let ext_cred = BasicCredential::new(es.credential_identity.clone());
        let external_sender = ExternalSender::new(es.signature_key.clone().into(), ext_cred.into());
        let extensions = Extensions::single(Extension::ExternalSenders(vec![external_sender]))
            .map_err(|e| format!("ext single: {e:?}"))?;

        let capabilities = Self::dave_capabilities();

        let config = MlsGroupCreateConfig::builder()
            .ciphersuite(CIPHERSUITE)
            .wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
            .use_ratchet_tree_extension(true)
            .capabilities(capabilities)
            .with_group_context_extensions(extensions)
            .build();

        let group = MlsGroup::new_with_group_id(
            &self.provider,
            &self.signer,
            &config,
            GroupId::from_slice(group_id),
            self.credential_with_key.clone(),
        )
        .map_err(|e| format!("group new: {e:?}"))?;

        self.group = Some(group);
        Ok(())
    }

    fn group_id_from_proposals(body: &[u8]) -> Option<Vec<u8>> {
        let mut o = 1usize;
        let (_vlen, n) = wire::mls_varint_decode(body.get(o..)?)?;
        o += n;
        o += 4;
        let (gid_len, n) = wire::mls_varint_decode(body.get(o..)?)?;
        o += n;
        let gid = body.get(o..o + gid_len as usize)?;
        Some(gid.to_vec())
    }

    pub fn ingest_proposals(&mut self, body: &[u8]) -> Result<(), BoxErr> {
        use tls_codec::Deserialize as _;

        if body.is_empty() {
            return Err("empty proposals body".into());
        }

        if self.group.is_none() {
            return Ok(());
        }
        let group = self.group.as_mut().ok_or("no local group")?;
        let op_type = body[0];
        let mut o = 1usize;
        if op_type != 0 {
            return Ok(());
        }
        let (vec_len, n) = wire::mls_varint_decode(&body[o..]).ok_or("proposals: bad vec len")?;
        o += n;
        let end = o + vec_len as usize;
        if end > body.len() {
            return Err("proposals: vec overflow".into());
        }

        let mut slice = &body[o..end];
        while !slice.is_empty() {
            let mut cur = std::io::Cursor::new(slice);
            let msg = MlsMessageIn::tls_deserialize(&mut cur)
                .map_err(|e| format!("proposal msg de: {e:?}"))?;
            let consumed = cur.position() as usize;
            slice = &slice[consumed..];

            let proto: ProtocolMessage = msg
                .try_into_protocol_message()
                .map_err(|e| format!("not a protocol msg: {e:?}"))?;
            let processed = group
                .process_message(&self.provider, proto)
                .map_err(|e| format!("process proposal: {e:?}"))?;
            if let ProcessedMessageContent::ProposalMessage(qp) = processed.into_content() {
                group
                    .store_pending_proposal(self.provider.storage(), *qp)
                    .map_err(|e| format!("store proposal: {e:?}"))?;
            }
        }
        Ok(())
    }

    pub fn is_joined(&self) -> bool {
        self.group.is_some()
    }

    pub fn has_pending(&self) -> bool {
        self.group
            .as_ref()
            .map(|g| g.has_pending_proposals())
            .unwrap_or(false)
    }

    pub fn commit_pending(&mut self) -> Result<Option<Vec<u8>>, BoxErr> {
        use tls_codec::Serialize as _;
        let Some(group) = self.group.as_mut() else {
            return Ok(None);
        };
        if !group.has_pending_proposals() {
            return Ok(None);
        }

        let (commit, welcome, _gi) = group
            .commit_to_pending_proposals(&self.provider, &self.signer)
            .map_err(|e| format!("commit: {e:?}"))?;

        let mut payload = commit
            .to_bytes()
            .map_err(|e| format!("commit ser: {e:?}"))?;
        if let Some(w) = welcome {
            if let MlsMessageBodyOut::Welcome(welcome) = w.body() {
                let wbytes = welcome
                    .tls_serialize_detached()
                    .map_err(|e| format!("welcome ser: {e:?}"))?;
                payload.extend_from_slice(&wbytes);
            }
        }
        Ok(Some(wire::frame_outbound(
            wire::op::MLS_COMMIT_WELCOME,
            &payload,
        )))
    }

    pub fn handle_announce_commit(&mut self, body: &[u8]) -> Result<(), BoxErr> {
        use tls_codec::Deserialize as _;

        let Some(group) = self.group.as_mut() else {
            return Ok(());
        };
        if group.pending_commit().is_some() {
            group
                .merge_pending_commit(&self.provider)
                .map_err(|e| format!("merge own pending commit: {e:?}"))?;
            crate::info!("DAVE: op29 = our own winning commit → merge_pending_commit");
            return Ok(());
        }
        crate::info!("DAVE: op29 = external commit → process+merge");
        let mut cur = std::io::Cursor::new(body);
        let msg =
            MlsMessageIn::tls_deserialize(&mut cur).map_err(|e| format!("commit de: {e:?}"))?;
        let proto: ProtocolMessage = msg
            .try_into_protocol_message()
            .map_err(|e| format!("not protocol msg: {e:?}"))?;
        let processed = group
            .process_message(&self.provider, proto)
            .map_err(|e| format!("process commit: {e:?}"))?;
        if let ProcessedMessageContent::StagedCommitMessage(sc) = processed.into_content() {
            group
                .merge_staged_commit(&self.provider, *sc)
                .map_err(|e| format!("merge commit: {e:?}"))?;
        }
        Ok(())
    }

    pub fn handle_welcome(&mut self, body: &[u8]) -> Result<(), BoxErr> {
        use tls_codec::Deserialize as _;

        if let Some(mut old) = self.group.take() {
            let _ = old.delete(self.provider.storage());
        }

        let mut cur = std::io::Cursor::new(body);
        let welcome =
            Welcome::tls_deserialize(&mut cur).map_err(|e| format!("welcome de: {e:?}"))?;
        let join_cfg = MlsGroupJoinConfig::builder()
            .wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
            .build();
        let staged = StagedWelcome::new_from_welcome(&self.provider, &join_cfg, welcome, None)
            .map_err(|e| format!("staged welcome: {e:?}"))?;
        let group = staged
            .into_group(&self.provider)
            .map_err(|e| format!("welcome into_group: {e:?}"))?;
        self.group = Some(group);
        Ok(())
    }

    pub fn group_member_ids(&self) -> Vec<u64> {
        let Some(g) = self.group.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for m in g.members() {
            if let Ok(bc) = BasicCredential::try_from(m.credential.clone()) {
                let id = bc.identity();
                if id.len() == 8 {
                    out.push(u64::from_be_bytes(id.try_into().unwrap()));
                }
            }
        }
        out
    }

    pub fn state_summary(&self) -> String {
        match self.group.as_ref() {
            None => "no group".into(),
            Some(g) => {
                let auth = g.epoch_authenticator().as_slice();
                let hex: String = auth.iter().map(|b| format!("{b:02x}")).collect();
                let members = g.members().count();
                format!(
                    "epoch={} members={} auth={hex}",
                    g.epoch().as_u64(),
                    members
                )
            }
        }
    }

    pub fn verification_code(&self) -> Option<String> {
        let g = self.group.as_ref()?;
        let auth = g.epoch_authenticator();
        let data = auth.as_slice();
        if data.len() < 30 {
            return None;
        }
        let mut out = String::new();
        for grp in 0..6 {
            let mut v: u64 = 0;
            for b in 0..5 {
                v = (v << 8) | data[grp * 5 + b] as u64;
            }
            v %= 100_000;
            out.push_str(&format!("{v:05} "));
        }
        Some(out.trim_end().to_string())
    }

    pub fn sender_base_secret(&self, sender_id: u64) -> Result<Vec<u8>, BoxErr> {
        let group = self.group.as_ref().ok_or("no group for export")?;
        let ctx = sender_id.to_le_bytes();
        group
            .export_secret(self.provider.crypto(), "Discord Secure Frames v0", &ctx, 16)
            .map_err(|e| format!("export secret: {e:?}").into())
    }

    pub fn sender_base_secret_ctx(
        &self,
        ctx: &[u8],
        label: &str,
    ) -> Result<Vec<u8>, BoxErr> {
        let group = self.group.as_ref().ok_or("no group for export")?;
        group
            .export_secret(self.provider.crypto(), label, ctx, 16)
            .map_err(|e| format!("export secret: {e:?}").into())
    }
}
