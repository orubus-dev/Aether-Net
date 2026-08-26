use crate::network::node::NodeId;
use ed25519_dalek::{
    Signature,
    Signer,
    SigningKey,
    Verifier,
    VerifyingKey,
};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use x25519_dalek::{
    PublicKey as X25519PublicKey,
    StaticSecret,
};
use zeroize::Zeroizing;

pub const EPHEMERAL_PUBLIC_KEY_LENGTH: usize = 32;
pub const SESSION_ID_LENGTH: usize = 16;

const KEY_LENGTH: usize = 32;
const NONCE_PREFIX_LENGTH: usize = 4;
const KEY_MATERIAL_LENGTH: usize = 104;

const LINK_DOMAIN: &[u8] = b"AETHER-LINK-V1";
const KDF_INFO: &[u8] = b"AETHER-LINK-SESSION-KEYS-V1";
const REKEY_INFO: &[u8] = b"AETHER-LINK-REKEY-V1";

#[derive(Debug)]
pub enum CryptoError {
    RandomnessUnavailable,
    InvalidKeyMaterial,
    KeyDerivationFailed,
    InvalidSignature,
    RekeyEpochExpired,
}

pub struct EphemeralKey {
    secret: StaticSecret,
    public: X25519PublicKey,
}

impl EphemeralKey {
    pub fn generate() -> Result<Self, CryptoError> {
        let mut secret_bytes = [0u8; 32];

        getrandom::fill(&mut secret_bytes)
            .map_err(|_| CryptoError::RandomnessUnavailable)?;

        let secret = StaticSecret::from(secret_bytes);
        let public = X25519PublicKey::from(&secret);

        Ok(Self { secret, public })
    }

    pub fn public_key(&self) -> X25519PublicKey {
        self.public
    }

    pub fn derive_shared_secret(
        &self,
        remote_public: &X25519PublicKey,
    ) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(
            self.secret
                .diffie_hellman(remote_public)
                .to_bytes(),
        )
    }
}

pub fn sign_key_exchange(
    signing_key: &SigningKey,
    node_id: NodeId,
    device_id: &[u8; 32],
    challenge: &[u8; 32],
    ephemeral_public: &X25519PublicKey,
    timestamp: u64,
) -> Signature {
    let transcript = key_exchange_transcript(
        node_id,
        device_id,
        challenge,
        ephemeral_public,
        timestamp,
    );

    signing_key.sign(&transcript)
}

pub fn verify_key_exchange(
    public_key: &VerifyingKey,
    node_id: NodeId,
    device_id: &[u8; 32],
    challenge: &[u8; 32],
    ephemeral_public: &X25519PublicKey,
    timestamp: u64,
    signature: &Signature,
) -> bool {
    let transcript = key_exchange_transcript(
        node_id,
        device_id,
        challenge,
        ephemeral_public,
        timestamp,
    );

    public_key
        .verify(&transcript, signature)
        .is_ok()
}

pub fn derive_session_keys(
    shared_secret: &[u8; 32],
    transcript_hash: &[u8; 32],
    initiator: NodeId,
    responder: NodeId,
    is_initiator: bool,
) -> Result<SessionKeys, CryptoError> {
    let hkdf =
        Hkdf::<Sha256>::new(
            Some(transcript_hash),
            shared_secret,
        );

    let mut material =
        [0u8; KEY_MATERIAL_LENGTH];

    hkdf.expand(
        KDF_INFO,
        &mut material,
    )
    .map_err(|_| CryptoError::KeyDerivationFailed)?;

    let mut initiator_key = [0u8; KEY_LENGTH];
    let mut responder_key = [0u8; KEY_LENGTH];

    let mut initiator_nonce =
        [0u8; NONCE_PREFIX_LENGTH];

    let mut responder_nonce =
        [0u8; NONCE_PREFIX_LENGTH];

    let mut rekey_secret =
        [0u8; KEY_LENGTH];

    initiator_key.copy_from_slice(
        &material[0..32],
    );

    responder_key.copy_from_slice(
        &material[32..64],
    );

    initiator_nonce.copy_from_slice(
        &material[64..68],
    );

    responder_nonce.copy_from_slice(
        &material[68..72],
    );

    rekey_secret.copy_from_slice(
        &material[72..104],
    );

    let session_id =
        session_id(
            transcript_hash,
            initiator,
            responder,
        );

    let (
        send_key,
        recv_key,
        send_nonce_prefix,
        recv_nonce_prefix,
    ) = directional_material(
        &initiator_key,
        &responder_key,
        &initiator_nonce,
        &responder_nonce,
        is_initiator,
    );

    Ok(SessionKeys {
        session_id,
        send_key: Zeroizing::new(send_key),
        recv_key: Zeroizing::new(recv_key),
        send_nonce_prefix,
        recv_nonce_prefix,
        rekey_secret: Zeroizing::new(rekey_secret),
        is_initiator,
    })
}

pub fn rekey_session_keys(
    current: &SessionKeys,
    next_epoch: u64,
) -> Result<SessionKeys, CryptoError> {
    if next_epoch == 0 {
        return Err(CryptoError::RekeyEpochExpired);
    }

    let hkdf =
        Hkdf::<Sha256>::new(
            Some(&current.session_id),
            current.rekey_secret.as_ref(),
        );

    let mut info =
        Vec::with_capacity(
            REKEY_INFO.len() + 8,
        );

    info.extend_from_slice(REKEY_INFO);
    info.extend_from_slice(
        &next_epoch.to_be_bytes(),
    );

    let mut material =
        [0u8; 72];

    hkdf.expand(
        &info,
        &mut material,
    )
    .map_err(|_| CryptoError::KeyDerivationFailed)?;

    let mut initiator_key =
        [0u8; KEY_LENGTH];

    let mut responder_key =
        [0u8; KEY_LENGTH];

    let mut initiator_nonce =
        [0u8; NONCE_PREFIX_LENGTH];

    let mut responder_nonce =
        [0u8; NONCE_PREFIX_LENGTH];

    initiator_key.copy_from_slice(
        &material[0..32],
    );

    responder_key.copy_from_slice(
        &material[32..64],
    );

    initiator_nonce.copy_from_slice(
        &material[64..68],
    );

    responder_nonce.copy_from_slice(
        &material[68..72],
    );

    let (
        send_key,
        recv_key,
        send_nonce_prefix,
        recv_nonce_prefix,
    ) = directional_material(
        &initiator_key,
        &responder_key,
        &initiator_nonce,
        &responder_nonce,
        current.is_initiator,
    );

    Ok(SessionKeys {
        session_id: current.session_id,
        send_key: Zeroizing::new(send_key),
        recv_key: Zeroizing::new(recv_key),
        send_nonce_prefix,
        recv_nonce_prefix,
        rekey_secret: current.rekey_secret.clone(),
        is_initiator: current.is_initiator,
    })
}

#[derive(Clone)]
pub struct SessionKeys {
    pub session_id: [u8; SESSION_ID_LENGTH],
    pub send_key: Zeroizing<[u8; KEY_LENGTH]>,
    pub recv_key: Zeroizing<[u8; KEY_LENGTH]>,
    pub send_nonce_prefix: [u8; NONCE_PREFIX_LENGTH],
    pub recv_nonce_prefix: [u8; NONCE_PREFIX_LENGTH],
    pub(crate) rekey_secret: Zeroizing<[u8; KEY_LENGTH]>,
    pub(crate) is_initiator: bool,
}

fn directional_material(
    initiator_key: &[u8; KEY_LENGTH],
    responder_key: &[u8; KEY_LENGTH],
    initiator_nonce: &[u8; NONCE_PREFIX_LENGTH],
    responder_nonce: &[u8; NONCE_PREFIX_LENGTH],
    is_initiator: bool,
) -> (
    [u8; KEY_LENGTH],
    [u8; KEY_LENGTH],
    [u8; NONCE_PREFIX_LENGTH],
    [u8; NONCE_PREFIX_LENGTH],
) {
    if is_initiator {
        (
            *initiator_key,
            *responder_key,
            *initiator_nonce,
            *responder_nonce,
        )
    } else {
        (
            *responder_key,
            *initiator_key,
            *responder_nonce,
            *initiator_nonce,
        )
    }
}

fn key_exchange_transcript(
    node_id: NodeId,
    device_id: &[u8; 32],
    challenge: &[u8; 32],
    ephemeral_public: &X25519PublicKey,
    timestamp: u64,
) -> Vec<u8> {
    let mut bytes =
        Vec::with_capacity(
            LINK_DOMAIN.len()
                + 32
                + 32
                + 32
                + 32
                + 8,
        );

    bytes.extend_from_slice(
        LINK_DOMAIN,
    );

    bytes.extend_from_slice(
        node_id.as_bytes(),
    );

    bytes.extend_from_slice(
        device_id,
    );

    bytes.extend_from_slice(
        challenge,
    );

    bytes.extend_from_slice(
        ephemeral_public.as_bytes(),
    );

    bytes.extend_from_slice(
        &timestamp.to_be_bytes(),
    );

    bytes
}

pub fn transcript_hash(
    transcript_parts: &[&[u8]],
) -> [u8; 32] {
    let mut hasher =
        Sha256::new();

    hasher.update(
        b"AETHER-LINK-TRANSCRIPT-V1",
    );

    for part in transcript_parts {
        hasher.update(part);
        hasher.update([0u8]);
    }

    hasher.finalize().into()
}

fn session_id(
    transcript_hash: &[u8; 32],
    initiator: NodeId,
    responder: NodeId,
) -> [u8; SESSION_ID_LENGTH] {
    let mut hasher =
        Sha256::new();

    hasher.update(
        b"AETHER-LINK-SESSION-ID-V1",
    );

    hasher.update(
        transcript_hash,
    );

    hasher.update(
        initiator.as_bytes(),
    );

    hasher.update(
        responder.as_bytes(),
    );

    let digest =
        hasher.finalize();

    let mut result =
        [0u8; SESSION_ID_LENGTH];

    result.copy_from_slice(
        &digest[..SESSION_ID_LENGTH],
    );

    result
}
