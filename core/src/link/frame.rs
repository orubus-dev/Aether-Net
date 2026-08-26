use crate::link::crypto::SessionKeys;
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
pub const LINK_HEADER_LENGTH: usize = 16;
pub const MAX_LINK_PAYLOAD: usize = 65_535;

#[derive(Debug)]
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
}

#[derive(Debug, Clone, Copy)]
pub struct FrameHeader {
    pub message_type: MessageType,
    pub priority: TrafficPriority,
    pub flags: PacketFlags,
    pub counter: u64,
    pub ciphertext_length: u32,
}

impl FrameHeader {
    pub fn encode(
        &self,
        session_id: &[u8; 16],
    ) -> Vec<u8> {
        let mut output =
            Vec::with_capacity(
                LINK_HEADER_LENGTH,
            );

        output.extend_from_slice(
            &LINK_MAGIC,
        );

        output.push(LINK_VERSION);
        output.push(
            self.message_type as u8,
        );
        output.push(
            self.priority as u8,
        );

        output.extend_from_slice(
            &self.flags.bits().to_be_bytes(),
        );

        output.extend_from_slice(
            &self.counter.to_be_bytes(),
        );

        output.extend_from_slice(
            &self.ciphertext_length.to_be_bytes(),
        );

        let _ = session_id;

        output
    }
}

pub struct LinkSession {
    keys: SessionKeys,
    send_counter: u64,
    receive_window: ReplayWindow,
}

impl LinkSession {
    pub fn new(keys: SessionKeys) -> Self {
        Self {
            keys,
            send_counter: 0,
            receive_window: ReplayWindow::new(),
        }
    }

    pub fn session_id(&self) -> &[u8; 16] {
        &self.keys.session_id
    }

    pub fn encrypt(
        &mut self,
        message_type: MessageType,
        priority: TrafficPriority,
        flags: PacketFlags,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, FrameError> {
        if plaintext.len() > MAX_LINK_PAYLOAD {
            return Err(FrameError::InvalidLength);
        }

        let counter =
            self.send_counter;

        let next_counter =
            counter
                .checked_add(1)
                .ok_or(FrameError::CounterExhausted)?;

        let nonce =
            build_nonce(
                &self.keys.send_nonce_prefix,
                counter,
            );

        let ciphertext_length =
            plaintext
                .len()
                .checked_add(16)
                .ok_or(FrameError::InvalidLength)?;

        if ciphertext_length > MAX_LINK_PAYLOAD {
            return Err(FrameError::InvalidLength);
        }

        let header =
            FrameHeader {
                message_type,
                priority,
                flags: flags.insert(
                    PacketFlags::ENCRYPTED,
                ),
                counter,
                ciphertext_length:
                    ciphertext_length as u32,
            };

        let header_bytes =
            header.encode(
                &self.keys.session_id,
            );

        let cipher =
            ChaCha20Poly1305::new_from_slice(
                self.keys.send_key.as_ref(),
            )
            .map_err(|_| FrameError::EncryptionFailed)?;

        let encrypted =
            cipher
                .encrypt(
                    Nonce::from_slice(&nonce),
                    Payload {
                        msg: plaintext,
                        aad: &header_bytes,
                    },
                )
                .map_err(|_| FrameError::EncryptionFailed)?;

        self.send_counter =
            next_counter;

        let mut output =
            header_bytes;

        output.extend_from_slice(
            &encrypted,
        );

        Ok(output)
    }

    pub fn decrypt(
        &mut self,
        frame: &[u8],
    ) -> Result<
        DecryptedFrame,
        FrameError,
    > {
        if frame.len()
            < LINK_HEADER_LENGTH
        {
            return Err(FrameError::InvalidHeader);
        }

        if frame[..4]
            != LINK_MAGIC
        {
            return Err(FrameError::InvalidMagic);
        }

        if frame[4]
            != LINK_VERSION
        {
            return Err(FrameError::InvalidVersion);
        }

        let message_type =
            MessageType::from_u8(
                frame[5],
            )
            .ok_or(
                FrameError::UnknownMessageType
            )?;

        let priority =
            TrafficPriority::from_u8(
                frame[6],
            )
            .ok_or(
                FrameError::UnknownPriority
            )?;

        let flags =
            PacketFlags(
                u16::from_be_bytes(
                    [frame[7], frame[8]],
                ),
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
            return Err(FrameError::InvalidFlags);
        }

        if !flags.contains(
            PacketFlags::ENCRYPTED,
        ) {
            return Err(FrameError::InvalidFlags);
        }

        let mut counter_bytes =
            [0u8; 8];

        counter_bytes.copy_from_slice(
            &frame[9..17],
        );

        let counter =
            u64::from_be_bytes(
                counter_bytes,
            );

        let length_start = 17;
        let length_end = 21;

        if frame.len()
            < length_end
        {
            return Err(FrameError::InvalidHeader);
        }

        let mut length_bytes =
            [0u8; 4];

        length_bytes.copy_from_slice(
            &frame[length_start..length_end],
        );

        let ciphertext_length =
            u32::from_be_bytes(
                length_bytes,
            ) as usize;

        if ciphertext_length > MAX_LINK_PAYLOAD {
            return Err(FrameError::InvalidLength);
        }

        if frame.len()
            != LINK_HEADER_LENGTH + 5 + ciphertext_length
        {
            return Err(FrameError::InvalidLength);
        }

        if !self
            .receive_window
            .accept(counter)
        {
            return Err(FrameError::ReplayDetected);
        }

        let header =
            &frame[..21];

        let encrypted =
            &frame[21..];

        let nonce =
            build_nonce(
                &self.keys.recv_nonce_prefix,
                counter,
            );

        let cipher =
            ChaCha20Poly1305::new_from_slice(
                self.keys.recv_key.as_ref(),
            )
            .map_err(|_| FrameError::DecryptionFailed)?;

        let plaintext =
            cipher
                .decrypt(
                    Nonce::from_slice(&nonce),
                    Payload {
                        msg: encrypted,
                        aad: header,
                    },
                )
                .map_err(|_| FrameError::DecryptionFailed)?;

        Ok(DecryptedFrame {
            message_type,
            priority,
            flags,
            counter,
            payload: plaintext,
        })
    }
}

#[derive(Debug)]
pub struct DecryptedFrame {
    pub message_type: MessageType,
    pub priority: TrafficPriority,
    pub flags: PacketFlags,
    pub counter: u64,
    pub payload: Vec<u8>,
}

fn build_nonce(
    prefix: &[u8; 4],
    counter: u64,
) -> [u8; 12] {
    let mut nonce =
        [0u8; 12];

    nonce[..4]
        .copy_from_slice(prefix);

    nonce[4..]
        .copy_from_slice(
            &counter.to_be_bytes(),
        );

    nonce
}
