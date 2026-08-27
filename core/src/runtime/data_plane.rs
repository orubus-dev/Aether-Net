use crate::link::{FrameError, LinkSession};
use crate::network::{AetherPacket, FlowId, MessageType, PacketFlags, RouteToken, TrafficPriority};

pub struct EncryptedDataPlane;

impl EncryptedDataPlane {
    pub fn encode_packet(
        session: &mut LinkSession,

        priority: TrafficPriority,

        flow: FlowId,

        route_token: RouteToken,

        hop_limit: u8,

        payload: Vec<u8>,
    ) -> Result<Vec<u8>, DataPlaneError> {
        let packet = AetherPacket::new(
            MessageType::Data,
            priority,
            PacketFlags::RELAYABLE,
            hop_limit,
            flow,
            route_token,
            payload,
        )
        .map_err(DataPlaneError::Packet)?;

        let plaintext = packet.encode().map_err(DataPlaneError::Packet)?;

        session
            .encrypt(
                MessageType::Data,
                priority,
                PacketFlags::RELAYABLE,
                &plaintext,
            )
            .map_err(DataPlaneError::Frame)
    }

    pub fn decode_packet(
        session: &mut LinkSession,

        frame: &[u8],
    ) -> Result<AetherPacket, DataPlaneError> {
        let decrypted = session.decrypt(frame).map_err(DataPlaneError::Frame)?;

        if decrypted.message_type != MessageType::Data {
            return Err(DataPlaneError::UnexpectedMessage);
        }

        AetherPacket::decode(&decrypted.payload).map_err(DataPlaneError::Packet)
    }
}

#[derive(Debug)]
pub enum DataPlaneError {
    Frame(FrameError),

    Packet(crate::network::PacketError),

    UnexpectedMessage,
}
