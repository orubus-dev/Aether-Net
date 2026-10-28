use super::{PeerRecord, ResourceRecord, ServiceRecord};
use crate::identity::device::{DeviceId, DeviceSigningKey};
use crate::identity::fingerprint::Fingerprint;
use crate::network::NodeId;
use crate::physical::PhysicalTransport;
use ed25519_dalek::{Signature, VerifyingKey};

pub const DISCOVERY_GOSSIP_MAGIC: [u8; 4] = *b"AEG1";
pub const DISCOVERY_GOSSIP_VERSION: u8 = 1;
pub const DISCOVERY_GOSSIP_SIGNATURE_LENGTH: usize = 64;
pub const MAX_ADVERTISEMENT_PEERS: usize = 32;
pub const MAX_ADVERTISEMENT_SERVICES: usize = 32;
pub const MAX_ADVERTISEMENT_RESOURCES: usize = 32;
pub const MAX_ADVERTISEMENT_AGE_SECONDS: u64 = 120;

const HEADER_LENGTH: usize = 4 + 1 + 32 + 32 + 32 + 8 + 8 + 8 + 1 + 1 + 1;
const PEER_LENGTH: usize = 32 + 32 + 1 + 8 + 8;
const SERVICE_LENGTH: usize = 16 + 32 + 1 + 2 + 8 + 8;
const RESOURCE_LENGTH: usize = 32 + 32 + 1 + 8 + 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedDiscoveryAdvertisement {
    pub origin: NodeId,
    pub device_id: DeviceId,
    pub public_key: [u8; 32],
    pub sequence: u64,
    pub issued_at: u64,
    pub expires_at: u64,
    pub peers: Vec<PeerRecord>,
    pub services: Vec<ServiceRecord>,
    pub resources: Vec<ResourceRecord>,
    pub signature: [u8; DISCOVERY_GOSSIP_SIGNATURE_LENGTH],
}

impl SignedDiscoveryAdvertisement {
    pub fn new(
        signing_key: &DeviceSigningKey,
        origin: NodeId,
        device_id: DeviceId,
        sequence: u64,
        issued_at: u64,
        expires_at: u64,
        peers: &[PeerRecord],
        services: &[ServiceRecord],
        resources: &[ResourceRecord],
    ) -> Result<Self, DiscoveryGossipError> {
        let verifying_key = signing_key.verifying_key();
        let public_key = verifying_key.to_bytes();
        if DeviceId::from_public_key(&verifying_key) != device_id
            || node_id_from_public_key(&public_key) != origin
        {
            return Err(DiscoveryGossipError::IdentityMismatch);
        }
        validate_interval(issued_at, expires_at, issued_at)?;
        if peers.len() > MAX_ADVERTISEMENT_PEERS
            || services.len() > MAX_ADVERTISEMENT_SERVICES
            || resources.len() > MAX_ADVERTISEMENT_RESOURCES
        {
            return Err(DiscoveryGossipError::CapacityExceeded);
        }

        let mut advertisement = Self {
            origin,
            device_id,
            public_key,
            sequence,
            issued_at,
            expires_at,
            peers: peers.to_vec(),
            services: services.to_vec(),
            resources: resources.to_vec(),
            signature: [0; DISCOVERY_GOSSIP_SIGNATURE_LENGTH],
        };
        advertisement.signature = signing_key.sign(&advertisement.signing_bytes()).to_bytes();
        Ok(advertisement)
    }

    pub fn verify(&self, now: u64) -> Result<(), DiscoveryGossipError> {
        if self.peers.len() > MAX_ADVERTISEMENT_PEERS
            || self.services.len() > MAX_ADVERTISEMENT_SERVICES
            || self.resources.len() > MAX_ADVERTISEMENT_RESOURCES
        {
            return Err(DiscoveryGossipError::CapacityExceeded);
        }
        let verifying_key = self.verifying_key()?;
        if DeviceId::from_public_key(&verifying_key) != self.device_id
            || node_id_from_public_key(&self.public_key) != self.origin
        {
            return Err(DiscoveryGossipError::IdentityMismatch);
        }
        validate_interval(self.issued_at, self.expires_at, now)?;
        if now.saturating_sub(self.issued_at) > MAX_ADVERTISEMENT_AGE_SECONDS {
            return Err(DiscoveryGossipError::Expired);
        }

        let signature = Signature::from_bytes(&self.signature);
        verifying_key
            .verify_strict(&self.signing_bytes(), &signature)
            .map_err(|_| DiscoveryGossipError::InvalidSignature)
    }

    pub fn encode(&self) -> Result<Vec<u8>, DiscoveryGossipError> {
        let mut bytes = self.signing_bytes();
        bytes.extend_from_slice(&self.signature);
        if bytes.len()
            > HEADER_LENGTH
                + MAX_ADVERTISEMENT_PEERS * PEER_LENGTH
                + MAX_ADVERTISEMENT_SERVICES * SERVICE_LENGTH
                + MAX_ADVERTISEMENT_RESOURCES * RESOURCE_LENGTH
                + DISCOVERY_GOSSIP_SIGNATURE_LENGTH
        {
            return Err(DiscoveryGossipError::CapacityExceeded);
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, DiscoveryGossipError> {
        if bytes.len() < HEADER_LENGTH + DISCOVERY_GOSSIP_SIGNATURE_LENGTH {
            return Err(DiscoveryGossipError::InvalidLength);
        }
        if bytes[0..4] != DISCOVERY_GOSSIP_MAGIC {
            return Err(DiscoveryGossipError::InvalidMagic);
        }
        if bytes[4] != DISCOVERY_GOSSIP_VERSION {
            return Err(DiscoveryGossipError::UnsupportedVersion);
        }

        let mut cursor = 5;
        let origin = read_node_id(bytes, &mut cursor)?;
        let device_id = read_device_id(bytes, &mut cursor)?;
        let mut public_key = [0u8; 32];
        read_exact(bytes, &mut cursor, &mut public_key)?;
        let sequence = read_u64(bytes, &mut cursor)?;
        let issued_at = read_u64(bytes, &mut cursor)?;
        let expires_at = read_u64(bytes, &mut cursor)?;
        let peer_count = read_u8(bytes, &mut cursor)? as usize;
        let service_count = read_u8(bytes, &mut cursor)? as usize;
        let resource_count = read_u8(bytes, &mut cursor)? as usize;
        if peer_count > MAX_ADVERTISEMENT_PEERS
            || service_count > MAX_ADVERTISEMENT_SERVICES
            || resource_count > MAX_ADVERTISEMENT_RESOURCES
        {
            return Err(DiscoveryGossipError::CapacityExceeded);
        }

        let expected = HEADER_LENGTH
            + peer_count * PEER_LENGTH
            + service_count * SERVICE_LENGTH
            + resource_count * RESOURCE_LENGTH
            + DISCOVERY_GOSSIP_SIGNATURE_LENGTH;
        if bytes.len() != expected {
            return Err(DiscoveryGossipError::InvalidLength);
        }

        let mut peers = Vec::with_capacity(peer_count);
        for _ in 0..peer_count {
            peers.push(decode_peer(bytes, &mut cursor)?);
        }
        let mut services = Vec::with_capacity(service_count);
        for _ in 0..service_count {
            services.push(decode_service(bytes, &mut cursor)?);
        }
        let mut resources = Vec::with_capacity(resource_count);
        for _ in 0..resource_count {
            resources.push(decode_resource(bytes, &mut cursor)?);
        }
        let mut signature = [0u8; DISCOVERY_GOSSIP_SIGNATURE_LENGTH];
        read_exact(bytes, &mut cursor, &mut signature)?;
        if cursor != bytes.len() {
            return Err(DiscoveryGossipError::InvalidLength);
        }

        Ok(Self {
            origin,
            device_id,
            public_key,
            sequence,
            issued_at,
            expires_at,
            peers,
            services,
            resources,
            signature,
        })
    }

    pub fn peer_records(&self) -> impl Iterator<Item = &PeerRecord> {
        self.peers.iter()
    }

    pub fn service_records(&self) -> impl Iterator<Item = &ServiceRecord> {
        self.services.iter()
    }

    pub fn resource_records(&self) -> impl Iterator<Item = &ResourceRecord> {
        self.resources.iter()
    }

    fn verifying_key(&self) -> Result<VerifyingKey, DiscoveryGossipError> {
        VerifyingKey::from_bytes(&self.public_key)
            .map_err(|_| DiscoveryGossipError::InvalidPublicKey)
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(
            HEADER_LENGTH
                + self.peers.len() * PEER_LENGTH
                + self.services.len() * SERVICE_LENGTH
                + self.resources.len() * RESOURCE_LENGTH,
        );
        bytes.extend_from_slice(&DISCOVERY_GOSSIP_MAGIC);
        bytes.push(DISCOVERY_GOSSIP_VERSION);
        bytes.extend_from_slice(self.origin.as_bytes());
        bytes.extend_from_slice(self.device_id.as_bytes());
        bytes.extend_from_slice(&self.public_key);
        bytes.extend_from_slice(&self.sequence.to_be_bytes());
        bytes.extend_from_slice(&self.issued_at.to_be_bytes());
        bytes.extend_from_slice(&self.expires_at.to_be_bytes());
        bytes.push(self.peers.len() as u8);
        bytes.push(self.services.len() as u8);
        bytes.push(self.resources.len() as u8);
        for peer in &self.peers {
            encode_peer(peer, &mut bytes);
        }
        for service in &self.services {
            encode_service(service, &mut bytes);
        }
        for resource in &self.resources {
            encode_resource(resource, &mut bytes);
        }
        bytes
    }
}

fn node_id_from_public_key(public_key: &[u8; 32]) -> NodeId {
    NodeId::from_bytes(*Fingerprint::from_domain(b"aether/node-id/v1", public_key).as_bytes())
}

fn validate_interval(
    issued_at: u64,
    expires_at: u64,
    now: u64,
) -> Result<(), DiscoveryGossipError> {
    if expires_at <= issued_at || now > expires_at || issued_at > now.saturating_add(5) {
        Err(DiscoveryGossipError::InvalidInterval)
    } else {
        Ok(())
    }
}

fn encode_peer(value: &PeerRecord, out: &mut Vec<u8>) {
    out.extend_from_slice(value.node_id.as_bytes());
    out.extend_from_slice(value.device_id.as_bytes());
    out.push(transport_to_u8(value.transport));
    out.extend_from_slice(&value.observed_at.to_be_bytes());
    out.extend_from_slice(&value.expires_at.to_be_bytes());
}

fn decode_peer(bytes: &[u8], cursor: &mut usize) -> Result<PeerRecord, DiscoveryGossipError> {
    let node_id = read_node_id(bytes, cursor)?;
    let device_id = read_device_id(bytes, cursor)?;
    let transport = transport_from_u8(read_u8(bytes, cursor)?)?;
    let observed_at = read_u64(bytes, cursor)?;
    let expires_at = read_u64(bytes, cursor)?;
    Ok(PeerRecord {
        node_id,
        device_id,
        transport,
        observed_at,
        expires_at,
    })
}

fn encode_service(value: &ServiceRecord, out: &mut Vec<u8>) {
    out.extend_from_slice(&value.service_id);
    out.extend_from_slice(value.owner.as_bytes());
    out.push(value.kind);
    out.extend_from_slice(&value.version.to_be_bytes());
    out.extend_from_slice(&value.observed_at.to_be_bytes());
    out.extend_from_slice(&value.expires_at.to_be_bytes());
}

fn decode_service(bytes: &[u8], cursor: &mut usize) -> Result<ServiceRecord, DiscoveryGossipError> {
    let mut service_id = [0u8; 16];
    read_exact(bytes, cursor, &mut service_id)?;
    let owner = read_node_id(bytes, cursor)?;
    let kind = read_u8(bytes, cursor)?;
    let version = read_u16(bytes, cursor)?;
    let observed_at = read_u64(bytes, cursor)?;
    let expires_at = read_u64(bytes, cursor)?;
    Ok(ServiceRecord {
        service_id,
        owner,
        kind,
        version,
        observed_at,
        expires_at,
    })
}

fn encode_resource(value: &ResourceRecord, out: &mut Vec<u8>) {
    out.extend_from_slice(&value.resource_id);
    out.extend_from_slice(value.provider.as_bytes());
    out.push(value.kind);
    out.extend_from_slice(&value.version.to_be_bytes());
    out.extend_from_slice(&value.observed_at.to_be_bytes());
    out.extend_from_slice(&value.expires_at.to_be_bytes());
}

fn decode_resource(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<ResourceRecord, DiscoveryGossipError> {
    let mut resource_id = [0u8; 32];
    read_exact(bytes, cursor, &mut resource_id)?;
    let provider = read_node_id(bytes, cursor)?;
    let kind = read_u8(bytes, cursor)?;
    let version = read_u64(bytes, cursor)?;
    let observed_at = read_u64(bytes, cursor)?;
    let expires_at = read_u64(bytes, cursor)?;
    Ok(ResourceRecord {
        resource_id,
        provider,
        kind,
        version,
        observed_at,
        expires_at,
    })
}

fn read_exact(
    bytes: &[u8],
    cursor: &mut usize,
    output: &mut [u8],
) -> Result<(), DiscoveryGossipError> {
    let end = cursor.saturating_add(output.len());
    if end > bytes.len() {
        return Err(DiscoveryGossipError::InvalidLength);
    }
    output.copy_from_slice(&bytes[*cursor..end]);
    *cursor = end;
    Ok(())
}

fn read_node_id(bytes: &[u8], cursor: &mut usize) -> Result<NodeId, DiscoveryGossipError> {
    let mut value = [0u8; 32];
    read_exact(bytes, cursor, &mut value)?;
    Ok(NodeId::from_bytes(value))
}

fn read_device_id(bytes: &[u8], cursor: &mut usize) -> Result<DeviceId, DiscoveryGossipError> {
    let mut value = [0u8; 32];
    read_exact(bytes, cursor, &mut value)?;
    Ok(DeviceId::from_bytes(value))
}

fn read_u8(bytes: &[u8], cursor: &mut usize) -> Result<u8, DiscoveryGossipError> {
    if *cursor >= bytes.len() {
        return Err(DiscoveryGossipError::InvalidLength);
    }
    let value = bytes[*cursor];
    *cursor += 1;
    Ok(value)
}

fn read_u16(bytes: &[u8], cursor: &mut usize) -> Result<u16, DiscoveryGossipError> {
    let mut value = [0u8; 2];
    read_exact(bytes, cursor, &mut value)?;
    Ok(u16::from_be_bytes(value))
}

fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, DiscoveryGossipError> {
    let mut value = [0u8; 8];
    read_exact(bytes, cursor, &mut value)?;
    Ok(u64::from_be_bytes(value))
}

fn transport_to_u8(value: PhysicalTransport) -> u8 {
    match value {
        PhysicalTransport::Ethernet => 1,
        PhysicalTransport::Wifi => 2,
        PhysicalTransport::WifiDirect => 7,
        PhysicalTransport::WifiAware => 8,
        PhysicalTransport::BluetoothLe => 3,
        PhysicalTransport::Uwb => 4,
        PhysicalTransport::LoRa => 9,
        PhysicalTransport::Cellular => 5,
        PhysicalTransport::Satellite => 10,
    }
}

fn transport_from_u8(value: u8) -> Result<PhysicalTransport, DiscoveryGossipError> {
    match value {
        1 => Ok(PhysicalTransport::Ethernet),
        2 => Ok(PhysicalTransport::Wifi),
        3 => Ok(PhysicalTransport::BluetoothLe),
        4 => Ok(PhysicalTransport::Uwb),
        5 => Ok(PhysicalTransport::Cellular),
        7 => Ok(PhysicalTransport::WifiDirect),
        8 => Ok(PhysicalTransport::WifiAware),
        9 => Ok(PhysicalTransport::LoRa),
        10 => Ok(PhysicalTransport::Satellite),
        _ => Err(DiscoveryGossipError::InvalidTransport),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryGossipError {
    InvalidLength,
    InvalidMagic,
    UnsupportedVersion,
    CapacityExceeded,
    InvalidPublicKey,
    InvalidSignature,
    IdentityMismatch,
    InvalidInterval,
    Expired,
    InvalidTransport,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_advertisement_round_trips_and_verifies() {
        let key = DeviceSigningKey::generate().unwrap();
        let device_id = DeviceId::from_public_key(&key.verifying_key());
        let origin = node_id_from_public_key(&key.verifying_key().to_bytes());
        let peer = PeerRecord::new(origin, device_id, PhysicalTransport::Ethernet, 100);
        let advertisement = SignedDiscoveryAdvertisement::new(
            &key,
            origin,
            device_id,
            7,
            100,
            180,
            &[peer],
            &[],
            &[],
        )
        .unwrap();
        let encoded = advertisement.encode().unwrap();
        let decoded = SignedDiscoveryAdvertisement::decode(&encoded).unwrap();
        decoded.verify(110).unwrap();
        assert_eq!(decoded.sequence, 7);
        assert_eq!(decoded.peer_records().count(), 1);
    }

    #[test]
    fn tampering_breaks_signature() {
        let key = DeviceSigningKey::generate().unwrap();
        let device_id = DeviceId::from_public_key(&key.verifying_key());
        let origin = node_id_from_public_key(&key.verifying_key().to_bytes());
        let advertisement =
            SignedDiscoveryAdvertisement::new(&key, origin, device_id, 1, 10, 50, &[], &[], &[])
                .unwrap();
        let mut encoded = advertisement.encode().unwrap();
        // Flip the tail of the signature: header bytes are covered by the
        // identity prefilter (IdentityMismatch) and the frame structure check.
        let tail = encoded.len() - 1;
        encoded[tail] ^= 1;
        let decoded = SignedDiscoveryAdvertisement::decode(&encoded).unwrap();
        assert_eq!(
            decoded.verify(20),
            Err(DiscoveryGossipError::InvalidSignature)
        );
    }

    #[test]
    fn identity_binding_is_self_certifying() {
        let key_a = DeviceSigningKey::generate().unwrap();
        let key_b = DeviceSigningKey::generate().unwrap();
        let device_a = DeviceId::from_public_key(&key_a.verifying_key());
        let origin_a = node_id_from_public_key(&key_a.verifying_key().to_bytes());
        assert!(
            SignedDiscoveryAdvertisement::new(&key_b, origin_a, device_a, 1, 1, 20, &[], &[], &[],)
                .is_err()
        );
    }
}
