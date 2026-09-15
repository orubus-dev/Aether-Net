use crate::link::{FrameError, LinkSession};
use crate::network::{AetherL3Packet, L3Error, MessageType, PacketFlags, TrafficPriority};

/// Bridges the native Aether L3 packet format into the existing authenticated
/// link/session machinery without introducing TCP/IP as a network dependency.
pub struct NativeAetherDataPlane;

impl NativeAetherDataPlane {
    pub fn encode(
        session: &mut LinkSession,
        packet: &AetherL3Packet,
    ) -> Result<Vec<u8>, NativeDataPlaneError> {
        let plaintext = packet.encode().map_err(NativeDataPlaneError::L3)?;
        session
            .encrypt(
                MessageType::Data,
                packet.priority,
                PacketFlags::RELAYABLE,
                &plaintext,
            )
            .map_err(NativeDataPlaneError::Frame)
    }

    pub fn decode(
        session: &mut LinkSession,
        frame: &[u8],
    ) -> Result<AetherL3Packet, NativeDataPlaneError> {
        let decrypted = session
            .decrypt(frame)
            .map_err(NativeDataPlaneError::Frame)?;
        if decrypted.message_type != MessageType::Data {
            return Err(NativeDataPlaneError::UnexpectedMessage);
        }
        AetherL3Packet::decode(&decrypted.payload).map_err(NativeDataPlaneError::L3)
    }

    pub fn priority_for_realtime() -> TrafficPriority {
        TrafficPriority::Voice
    }
}

#[derive(Debug)]
pub enum NativeDataPlaneError {
    Frame(FrameError),
    L3(L3Error),
    UnexpectedMessage,
}
