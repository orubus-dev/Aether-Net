use crate::link::crypto::{
    rekey_session_keys,
    SessionKeys,
};
use crate::link::replay::ReplayWindow;
use crate::network::protocol::{
    MessageType,
    PacketFlags,
    TrafficPriority,
};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    ChaCha20Poly1305,
    Nonce,
};

pub const LINK_MAGIC: [u8; 4] = *b"AEL1";
pub const LINK_VERSION: u8 = 1;
pub const LINK_HEADER_LENGTH: usize = 21;
pub const MAX_LINK_PAYLOAD: usize = 65_535;

const NONCE_PREFIX_LENGTH: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    InvalidHeader,
    InvalidMagic,
    InvalidVersion,
    UnknownMessageType,
    UnknownPriority,
    InvalidFlags,
    InvalidLength,
    ReplayDetected,
    EncryptionFailed,
    DecryptionFailed,
    CounterExhausted,
    InvalidEpoch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    pub message_type: MessageType,
    pub priority: TrafficPriority,
    pub flags: PacketFlags,
    pub counter: u64,
    pub epoch: u64,
    pub ciphertext_length: u32,
}

impl FrameHeader {
    pub const ENCODED_LENGTH: usize = LINK_HEADER_LENGTH;

    pub fn encode(&self) -> [u8; LINK_HEADER_LENGTH] {
        let mut output =
            [0u8; LINK_HEADER_LENGTH];

        output[0..4]
            .copy_from_slice(
                &LINK_MAGIC,
            );

        output[4] =
            LINK_VERSION;

        output[5] =
            self.message_type as u8;

        output[6] =
            self.priority as u8;

        output[7..9]
            .copy_from_slice(
                &self.flags.bits()
                    .to_be_bytes(),
            );

        output[9..17]
            .copy_from_slice(
                &self.counter
                    .to_be_bytes(),
            );

        output[17..21]
            .copy_from_slice(
                &self.ciphertext_length
                    .to_be_bytes(),
            );

        output
    }
}

pub struct LinkSession {
    keys: SessionKeys,
    send_counter: u64,
    receive_window: ReplayWindow,
    epoch: u64,
}

impl LinkSession {
    pub fn new(
        keys: SessionKeys,
    ) -> Self {
        Self {
            keys,
            send_counter: 0,
            receive_window:
                ReplayWindow::new(),
            epoch: 0,
        }
    }

    pub fn session_id(
        &self,
    ) -> &[u8; 16] {
        &self.keys.session_id
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn encrypt(
        &mut self,
        message_type: MessageType,
        priority: TrafficPriority,
        flags: PacketFlags,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, FrameError> {
        if plaintext.len()
            > MAX_LINK_PAYLOAD - 16
        {
            return Err(
                FrameError::InvalidLength
            );
        }

        let counter =
            self.send_counter;

        let next_counter =
            counter
                .checked_add(1)
                .ok_or(
                    FrameError::CounterExhausted
                )?;

        let nonce =
            build_nonce(
                &self.keys.send_nonce_prefix,
                counter,
            );

        let cipher =
            ChaCha20Poly1305::new_from_slice(
                self.keys.send_key.as_ref(),
            )
            .map_err(
                |_| FrameError::EncryptionFailed
            )?;

        let ciphertext_length =
            plaintext
                .len()
                .checked_add(16)
                .ok_or(
                    FrameError::InvalidLength
                )?;

        let header =
            FrameHeader {
                message_type,
                priority,
                flags: flags.insert(
                    PacketFlags::ENCRYPTED,
                ),
                counter,
                epoch: self.epoch,
                ciphertext_length:
                    ciphertext_length as u32,
            };

        let header_bytes =
            header.encode();

        let ciphertext =
            cipher.encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: &header_bytes,
                },
            )
            .map_err(
                |_| FrameError::EncryptionFailed
            )?;

        self.send_counter =
            next_counter;

        let mut output =
            Vec::with_capacity(
                LINK_HEADER_LENGTH
                    + ciphertext.len(),
            );

        output.extend_from_slice(
            &header_bytes,
        );

        output.extend_from_slice(
            &ciphertext,
        );

        Ok(output)
    }

    pub fn decrypt(
        &mut self,
        frame: &[u8],
    ) -> Result<DecryptedFrame, FrameError> {
        if frame.len()
            < LINK_HEADER_LENGTH
        {
            return Err(
                FrameError::InvalidHeader
            );
        }

        let header_bytes =
            &frame[..LINK_HEADER_LENGTH];

        if header_bytes[0..4]
            != LINK_MAGIC
        {
            return Err(
                FrameError::InvalidMagic
            );
        }

        if header_bytes[4]
            != LINK_VERSION
        {
            return Err(
                FrameError::InvalidVersion
            );
        }

        let message_type =
            MessageType::from_u8(
                header_bytes[5],
            )
            .ok_or(
                FrameError::UnknownMessageType
            )?;

        let priority =
            TrafficPriority::from_u8(
                header_bytes[6],
            )
            .ok_or(
                FrameError::UnknownPriority
            )?;

        let flags =
            PacketFlags(
                u16::from_be_bytes([
                    header_bytes[7],
                    header_bytes[8],
                ]),
            );

        let known_flags =
            PacketFlags::ENCRYPTED.0
                | PacketFlags::RELAYABLE.0
                | PacketFlags::ACK_REQUIRED.0
                | PacketFlags::FRAGMENT.0
                | PacketFlags::FINAL_FRAGMENT.0;

        if flags.bits()
            & !known_flags
            != 0
        {
            return Err(
                FrameError::InvalidFlags
            );
        }

        if !flags.contains(
            PacketFlags::ENCRYPTED,
        ) {
            return Err(
                FrameError::InvalidFlags
            );
        }

        let counter =
            u64::from_be_bytes(
                header_bytes[9..17]
                    .try_into()
                    .map_err(
                        |_| FrameError::InvalidHeader
                    )?,
            );

        let ciphertext_length =
            u32::from_be_bytes(
                header_bytes[17..21]
                    .try_into()
                    .map_err(
                        |_| FrameError::InvalidHeader
                    )?,
            ) as usize;

        if ciphertext_length
            > MAX_LINK_PAYLOAD
        {
            return Err(
                FrameError::InvalidLength
            );
        }

        if frame.len()
            != LINK_HEADER_LENGTH
                + ciphertext_length
        {
            return Err(
                FrameError::InvalidLength
            );
        }

        let epoch =
            self.read_epoch(
                header_bytes,
            );

        if epoch != self.epoch {
            return Err(
                FrameError::InvalidEpoch
            );
        }

        if !self.receive_window
            .accept(counter)
        {
            return Err(
                FrameError::ReplayDetected
            );
        }

        let encrypted =
            &frame[
                LINK_HEADER_LENGTH..
            ];

        let nonce =
            build_nonce(
                &self.keys.recv_nonce_prefix,
                counter,
            );

        let cipher =
            ChaCha20Poly1305::new_from_slice(
                self.keys.recv_key.as_ref(),
            )
            .map_err(
                |_| FrameError::DecryptionFailed
            )?;

        let plaintext =
            cipher.decrypt(
                Nonce::from_slice(
                    &nonce,
                ),
                Payload {
                    msg: encrypted,
                    aad: header_bytes,
                },
            )
            .map_err(
                |_| FrameError::DecryptionFailed
            )?;

        Ok(DecryptedFrame {
            message_type,
            priority,
            flags,
            counter,
            epoch,
            payload: plaintext,
        })
    }

    /// Advance both sides to the next session-key epoch.
    ///
    /// Both peers must perform this operation with the same epoch before
    /// exchanging frames using the new keys.
    pub fn rekey(
        &mut self,
        next_epoch: u64,
    ) -> Result<(), FrameError> {
        if next_epoch <= self.epoch {
            return Err(
                FrameError::InvalidEpoch
            );
        }

        self.keys =
            rekey_session_keys(
                &self.keys,
                next_epoch,
            )
            .map_err(
                |_| FrameError::InvalidEpoch
            )?;

        self.send_counter = 0;
        self.receive_window.reset();
        self.epoch = next_epoch;

        Ok(())
    }

    fn read_epoch(
        &self,
        header: &[u8],
    ) -> u64 {
        // Epoch is encoded into the payload-control extension in later
        // protocol versions. Current v1 uses the active session epoch.
        //
        // This method deliberately keeps the v1 wire layout stable while
        // reserving explicit epoch management at the session layer.
        let _ = header;
        self.epoch
    }
}

#[derive(Debug)]
pub struct DecryptedFrame {
    pub message_type: MessageType,
    pub priority: TrafficPriority,
    pub flags: PacketFlags,
    pub counter: u64,
    pub epoch: u64,
    pub payload: Vec<u8>,
}

fn build_nonce(
    prefix: &[u8; NONCE_PREFIX_LENGTH],
    counter: u64,
) -> [u8; 12] {
    let mut nonce =
        [0u8; 12];

    nonce[..NONCE_PREFIX_LENGTH]
        .copy_from_slice(
            prefix,
        );

    nonce[NONCE_PREFIX_LENGTH..]
        .copy_from_slice(
            &counter.to_be_bytes(),
        );

    nonce
}
