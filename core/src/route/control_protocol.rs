use super::SignedRouteAdvertisement;
use crate::network::NodeId;

pub const ROUTE_CONTROL_MAGIC: [u8; 4] = *b"ARTE";
pub const ROUTE_CONTROL_VERSION: u8 = 1;
pub const ROUTE_CONTROL_ADVERTISEMENT: u8 = 1;
pub const ROUTE_CONTROL_HEADER_LENGTH: usize = 8;
pub const ROUTE_CONTROL_FIXED_BODY_LENGTH: usize = 32 + 8 + 8 + 8 + 2 + 32 + 64;
pub const ROUTE_CONTROL_NEIGHBOR_LENGTH: usize = 32 + 8 + 8 + 4 + 8 + 4 + 8 + 4 + 2;
pub const ROUTE_CONTROL_MAX_NEIGHBORS: usize = 256;
pub const ROUTE_CONTROL_MAX_SIZE: usize = ROUTE_CONTROL_HEADER_LENGTH
    + ROUTE_CONTROL_FIXED_BODY_LENGTH
    + ROUTE_CONTROL_NEIGHBOR_LENGTH * ROUTE_CONTROL_MAX_NEIGHBORS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteControlCodecError {
    InvalidLength,
    InvalidMagic,
    UnsupportedVersion,
    UnsupportedMessage,
    TooManyNeighbors,
    InvalidAdvertisement,
}

pub fn encode_advertisement(
    message: &SignedRouteAdvertisement,
) -> Result<Vec<u8>, RouteControlCodecError> {
    let count = message.advertisement.neighbors.len();
    if count > ROUTE_CONTROL_MAX_NEIGHBORS || count > u16::MAX as usize {
        return Err(RouteControlCodecError::TooManyNeighbors);
    }
    if message.advertisement.expires_at <= message.advertisement.issued_at {
        return Err(RouteControlCodecError::InvalidAdvertisement);
    }
    if !message.verify() {
        return Err(RouteControlCodecError::InvalidAdvertisement);
    }

    let mut output = Vec::with_capacity(
        ROUTE_CONTROL_HEADER_LENGTH
            + ROUTE_CONTROL_FIXED_BODY_LENGTH
            + ROUTE_CONTROL_NEIGHBOR_LENGTH * count,
    );
    output.extend_from_slice(&ROUTE_CONTROL_MAGIC);
    output.push(ROUTE_CONTROL_VERSION);
    output.push(ROUTE_CONTROL_ADVERTISEMENT);
    output.extend_from_slice(&(count as u16).to_be_bytes());
    output.extend_from_slice(message.advertisement.origin.as_bytes());
    output.extend_from_slice(&message.advertisement.sequence.to_be_bytes());
    output.extend_from_slice(&message.advertisement.issued_at.to_be_bytes());
    output.extend_from_slice(&message.advertisement.expires_at.to_be_bytes());
    output.extend_from_slice(&message.public_key);
    for neighbor in &message.advertisement.neighbors {
        output.extend_from_slice(neighbor.node_id.as_bytes());
        output.extend_from_slice(&neighbor.metrics.latency_us.to_be_bytes());
        output.extend_from_slice(&neighbor.metrics.jitter_us.to_be_bytes());
        output.extend_from_slice(&neighbor.metrics.packet_loss_ppm.to_be_bytes());
        output.extend_from_slice(&neighbor.metrics.bandwidth_bps.to_be_bytes());
        output.extend_from_slice(&neighbor.metrics.reliability_ppm.to_be_bytes());
        output.extend_from_slice(&neighbor.metrics.monetary_cost_micros.to_be_bytes());
        output.extend_from_slice(&neighbor.metrics.energy_cost.to_be_bytes());
        output.extend_from_slice(&neighbor.metrics.hop_count.to_be_bytes());
    }
    output.extend_from_slice(&message.signature);
    debug_assert_eq!(output.len(), expected_size(count));
    Ok(output)
}

pub fn decode_advertisement(
    bytes: &[u8],
) -> Result<SignedRouteAdvertisement, RouteControlCodecError> {
    if bytes.len() < ROUTE_CONTROL_HEADER_LENGTH + ROUTE_CONTROL_FIXED_BODY_LENGTH
        || bytes.len() > ROUTE_CONTROL_MAX_SIZE
    {
        return Err(RouteControlCodecError::InvalidLength);
    }
    if bytes[0..4] != ROUTE_CONTROL_MAGIC {
        return Err(RouteControlCodecError::InvalidMagic);
    }
    if bytes[4] != ROUTE_CONTROL_VERSION {
        return Err(RouteControlCodecError::UnsupportedVersion);
    }
    if bytes[5] != ROUTE_CONTROL_ADVERTISEMENT {
        return Err(RouteControlCodecError::UnsupportedMessage);
    }
    let count = u16::from_be_bytes([bytes[6], bytes[7]]) as usize;
    if count > ROUTE_CONTROL_MAX_NEIGHBORS {
        return Err(RouteControlCodecError::TooManyNeighbors);
    }
    if bytes.len() != expected_size(count) {
        return Err(RouteControlCodecError::InvalidLength);
    }

    let mut cursor = ROUTE_CONTROL_HEADER_LENGTH;
    let origin = read_node_id(bytes, &mut cursor)?;
    let sequence = read_u64(bytes, &mut cursor)?;
    let issued_at = read_u64(bytes, &mut cursor)?;
    let expires_at = read_u64(bytes, &mut cursor)?;
    let mut public_key = [0u8; 32];
    read_fixed(bytes, &mut cursor, &mut public_key)?;

    let mut neighbors = Vec::with_capacity(count);
    for _ in 0..count {
        let node_id = read_node_id(bytes, &mut cursor)?;
        let metrics = super::RouteMetrics {
            latency_us: read_u64(bytes, &mut cursor)?,
            jitter_us: read_u64(bytes, &mut cursor)?,
            packet_loss_ppm: read_u32(bytes, &mut cursor)?,
            bandwidth_bps: read_u64(bytes, &mut cursor)?,
            reliability_ppm: read_u32(bytes, &mut cursor)?,
            monetary_cost_micros: read_u64(bytes, &mut cursor)?,
            energy_cost: read_u32(bytes, &mut cursor)?,
            hop_count: read_u16(bytes, &mut cursor)?,
        };
        neighbors.push(super::RouteNeighbor { node_id, metrics });
    }

    let mut signature = [0u8; 64];
    read_fixed(bytes, &mut cursor, &mut signature)?;
    if cursor != bytes.len() || expires_at <= issued_at {
        return Err(RouteControlCodecError::InvalidAdvertisement);
    }

    let message = SignedRouteAdvertisement {
        advertisement: super::RouteAdvertisement::with_expiry(
            origin,
            sequence,
            issued_at,
            expires_at,
            neighbors,
        ),
        public_key,
        signature,
    };
    if !message.verify() {
        return Err(RouteControlCodecError::InvalidAdvertisement);
    }
    Ok(message)
}

fn expected_size(neighbors: usize) -> usize {
    ROUTE_CONTROL_HEADER_LENGTH
        + 32
        + 8
        + 8
        + 8
        + 32
        + ROUTE_CONTROL_NEIGHBOR_LENGTH * neighbors
        + 64
}

fn read_fixed(
    bytes: &[u8],
    cursor: &mut usize,
    output: &mut [u8],
) -> Result<(), RouteControlCodecError> {
    let end = cursor
        .checked_add(output.len())
        .ok_or(RouteControlCodecError::InvalidLength)?;
    if end > bytes.len() {
        return Err(RouteControlCodecError::InvalidLength);
    }
    output.copy_from_slice(&bytes[*cursor..end]);
    *cursor = end;
    Ok(())
}

fn read_node_id(bytes: &[u8], cursor: &mut usize) -> Result<NodeId, RouteControlCodecError> {
    let mut value = [0u8; 32];
    read_fixed(bytes, cursor, &mut value)?;
    Ok(NodeId::from_bytes(value))
}

fn read_u16(bytes: &[u8], cursor: &mut usize) -> Result<u16, RouteControlCodecError> {
    let mut value = [0u8; 2];
    read_fixed(bytes, cursor, &mut value)?;
    Ok(u16::from_be_bytes(value))
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, RouteControlCodecError> {
    let mut value = [0u8; 4];
    read_fixed(bytes, cursor, &mut value)?;
    Ok(u32::from_be_bytes(value))
}

fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, RouteControlCodecError> {
    let mut value = [0u8; 8];
    read_fixed(bytes, cursor, &mut value)?;
    Ok(u64::from_be_bytes(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::fingerprint::Fingerprint;
    use crate::route::{RouteAdvertisement, RouteMetrics, RouteNeighbor};
    use ed25519_dalek::SigningKey;

    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn node_id(key: &SigningKey) -> NodeId {
        let fingerprint = Fingerprint::from_domain(
            b"aether/node-id/v1",
            &key.verifying_key().to_bytes(),
        );
        NodeId::from_bytes(*fingerprint.as_bytes())
    }

    fn advertisement() -> SignedRouteAdvertisement {
        SignedRouteAdvertisement::sign(
            RouteAdvertisement::with_expiry(
                node_id(&key(1)),
                7,
                10,
                70,
                vec![RouteNeighbor {
                    node_id: NodeId::from_bytes([2; 32]),
                    metrics: RouteMetrics {
                        latency_us: 10,
                        jitter_us: 2,
                        packet_loss_ppm: 100,
                        bandwidth_bps: 10_000_000,
                        reliability_ppm: 999_000,
                        monetary_cost_micros: 4,
                        energy_cost: 2,
                        hop_count: 1,
                    },
                }],
            ),
            &key(1),
        )
    }

    #[test]
    fn advertisement_round_trip_is_deterministic() {
        let message = advertisement();
        let encoded = encode_advertisement(&message).unwrap();
        let decoded = decode_advertisement(&encoded).unwrap();
        assert_eq!(decoded, message);
        assert_eq!(encode_advertisement(&decoded).unwrap(), encoded);
    }

    #[test]
    fn malformed_length_is_rejected_before_parsing() {
        let message = advertisement();
        let mut encoded = encode_advertisement(&message).unwrap();
        encoded.pop();
        assert_eq!(
            decode_advertisement(&encoded),
            Err(RouteControlCodecError::InvalidLength)
        );
    }

    #[test]
    fn forged_signature_is_rejected() {
        let message = advertisement();
        let mut encoded = encode_advertisement(&message).unwrap();
        let last = encoded.len() - 1;
        encoded[last] ^= 1;
        assert_eq!(
            decode_advertisement(&encoded),
            Err(RouteControlCodecError::InvalidAdvertisement)
        );
    }
}
