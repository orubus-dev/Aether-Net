use crate::name_resolution::{NameDelegation, SignedNameDelegation};
use crate::naming::{
    NAME_MAX_LEN, NameError, NameHash, NameRecord, ResourceId, SIGNATURE_LENGTH, canonicalize_name,
};

pub const NAMING_MAGIC: [u8; 4] = *b"AEN1";
pub const NAMING_VERSION: u8 = 1;
pub const REQUEST_ID_LENGTH: usize = 16;
pub const MAX_NAME_ANNOUNCEMENT: usize = NAME_MAX_LEN;
pub const MAX_DELEGATION_LENGTH: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameQuery {
    pub request_id: [u8; REQUEST_ID_LENGTH],
    pub name_hash: NameHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameResponse {
    pub request_id: [u8; REQUEST_ID_LENGTH],
    pub name: Option<String>,
    pub record: Option<NameRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameAnnouncement {
    pub name: String,
    pub record: NameRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegationAnnouncement {
    pub delegation: SignedNameDelegation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameControl {
    Query(NameQuery),
    Response(NameResponse),
    Announcement(NameAnnouncement),
    Delegation(DelegationAnnouncement),
}

impl NameQuery {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(54);
        header(1, &mut out);
        out.extend_from_slice(&self.request_id);
        out.extend_from_slice(self.name_hash.as_bytes());
        out
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, NamingCodecError> {
        if bytes.len() != 54 {
            return Err(NamingCodecError::InvalidLength);
        };
        validate_header(bytes, 1)?;
        let mut request_id = [0; 16];
        request_id.copy_from_slice(&bytes[6..22]);
        let mut hash = [0; 32];
        hash.copy_from_slice(&bytes[22..54]);
        Ok(Self {
            request_id,
            name_hash: NameHash::from_bytes(hash),
        })
    }
}

impl NameResponse {
    pub fn encode(&self) -> Result<Vec<u8>, NamingCodecError> {
        let mut out = Vec::new();
        header(2, &mut out);
        out.extend_from_slice(&self.request_id);
        match (&self.name, &self.record) {
            (None, None) => out.push(0),
            (Some(name), Some(record)) => {
                let canonical = canonicalize_name(name).map_err(NamingCodecError::Name)?;
                if canonical.len() > NAME_MAX_LEN || NameHash::of(&canonical) != record.name_hash {
                    return Err(NamingCodecError::InvalidRecord);
                };
                out.push(1);
                out.push(canonical.len() as u8);
                out.extend_from_slice(canonical.as_bytes());
                encode_record(record, &mut out);
            }
            _ => return Err(NamingCodecError::InvalidRecord),
        }
        Ok(out)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, NamingCodecError> {
        if bytes.len() < 23 {
            return Err(NamingCodecError::InvalidLength);
        };
        validate_header(bytes, 2)?;
        let mut request_id = [0; 16];
        request_id.copy_from_slice(&bytes[6..22]);
        match bytes[22] {
            0 => {
                if bytes.len() != 23 {
                    return Err(NamingCodecError::InvalidLength);
                };
                Ok(Self {
                    request_id,
                    name: None,
                    record: None,
                })
            }
            1 => {
                if bytes.len() < 24 {
                    return Err(NamingCodecError::InvalidLength);
                };
                let len = bytes[23] as usize;
                if len == 0 || len > NAME_MAX_LEN || bytes.len() != 24 + len + 184 {
                    return Err(NamingCodecError::InvalidLength);
                };
                let name = String::from_utf8(bytes[24..24 + len].to_vec())
                    .map_err(|_| NamingCodecError::InvalidName)?;
                let canonical = canonicalize_name(&name).map_err(NamingCodecError::Name)?;
                let record = decode_record(&bytes[24 + len..])?;
                if NameHash::of(&canonical) != record.name_hash {
                    return Err(NamingCodecError::InvalidRecord);
                };
                Ok(Self {
                    request_id,
                    name: Some(canonical),
                    record: Some(record),
                })
            }
            _ => Err(NamingCodecError::InvalidFlag),
        }
    }
}

impl NameAnnouncement {
    pub fn encode(&self) -> Result<Vec<u8>, NamingCodecError> {
        let canonical = canonicalize_name(&self.name).map_err(NamingCodecError::Name)?;
        if canonical.len() > MAX_NAME_ANNOUNCEMENT
            || NameHash::of(&canonical) != self.record.name_hash
        {
            return Err(NamingCodecError::InvalidRecord);
        };
        let mut out = Vec::with_capacity(7 + canonical.len() + 184);
        header(3, &mut out);
        out.push(canonical.len() as u8);
        out.extend_from_slice(canonical.as_bytes());
        encode_record(&self.record, &mut out);
        Ok(out)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, NamingCodecError> {
        if bytes.len() < 7 {
            return Err(NamingCodecError::InvalidLength);
        };
        validate_header(bytes, 3)?;
        let len = bytes[6] as usize;
        if len == 0 || len > MAX_NAME_ANNOUNCEMENT || bytes.len() != 7 + len + 184 {
            return Err(NamingCodecError::InvalidLength);
        };
        let name = String::from_utf8(bytes[7..7 + len].to_vec())
            .map_err(|_| NamingCodecError::InvalidName)?;
        let canonical = canonicalize_name(&name).map_err(NamingCodecError::Name)?;
        let record = decode_record(&bytes[7 + len..])?;
        if NameHash::of(&canonical) != record.name_hash {
            return Err(NamingCodecError::InvalidRecord);
        };
        Ok(Self {
            name: canonical,
            record,
        })
    }
}

impl NameControl {
    pub fn encode(&self) -> Result<Vec<u8>, NamingCodecError> {
        match self {
            Self::Query(v) => Ok(v.encode()),
            Self::Response(v) => v.encode(),
            Self::Announcement(v) => v.encode(),
            Self::Delegation(v) => encode_delegation(&v.delegation),
        }
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, NamingCodecError> {
        if bytes.len() < 6 {
            return Err(NamingCodecError::InvalidLength);
        };
        match bytes[5] {
            1 => Ok(Self::Query(NameQuery::decode(bytes)?)),
            2 => Ok(Self::Response(NameResponse::decode(bytes)?)),
            3 => Ok(Self::Announcement(NameAnnouncement::decode(bytes)?)),
            4 => Ok(Self::Delegation(DelegationAnnouncement {
                delegation: decode_delegation(bytes)?,
            })),
            _ => Err(NamingCodecError::UnknownMessage),
        }
    }
}

fn header(kind: u8, out: &mut Vec<u8>) {
    out.extend_from_slice(&NAMING_MAGIC);
    out.push(NAMING_VERSION);
    out.push(kind)
}
fn validate_header(bytes: &[u8], kind: u8) -> Result<(), NamingCodecError> {
    if bytes.len() < 6 {
        return Err(NamingCodecError::InvalidLength);
    };
    if bytes[0..4] != NAMING_MAGIC {
        return Err(NamingCodecError::InvalidMagic);
    };
    if bytes[4] != NAMING_VERSION {
        return Err(NamingCodecError::UnsupportedVersion);
    };
    if bytes[5] != kind {
        return Err(NamingCodecError::UnknownMessage);
    };
    Ok(())
}
fn encode_record(record: &NameRecord, out: &mut Vec<u8>) {
    out.extend_from_slice(record.name_hash.as_bytes());
    out.extend_from_slice(record.resource_id.as_bytes());
    out.extend_from_slice(&record.owner_public_key);
    out.extend_from_slice(&record.version.to_be_bytes());
    out.extend_from_slice(&record.issued_at.to_be_bytes());
    out.extend_from_slice(&record.expires_at.to_be_bytes());
    out.extend_from_slice(&record.signature)
}
fn decode_record(bytes: &[u8]) -> Result<NameRecord, NamingCodecError> {
    if bytes.len() != 184 {
        return Err(NamingCodecError::InvalidLength);
    };
    let mut hash = [0; 32];
    hash.copy_from_slice(&bytes[0..32]);
    let mut resource = [0; 32];
    resource.copy_from_slice(&bytes[32..64]);
    let mut key = [0; 32];
    key.copy_from_slice(&bytes[64..96]);
    let version = u64::from_be_bytes(
        bytes[96..104]
            .try_into()
            .map_err(|_| NamingCodecError::InvalidLength)?,
    );
    let issued_at = u64::from_be_bytes(
        bytes[104..112]
            .try_into()
            .map_err(|_| NamingCodecError::InvalidLength)?,
    );
    let expires_at = u64::from_be_bytes(
        bytes[112..120]
            .try_into()
            .map_err(|_| NamingCodecError::InvalidLength)?,
    );
    let mut signature = [0; SIGNATURE_LENGTH];
    signature.copy_from_slice(&bytes[120..184]);
    Ok(NameRecord {
        name_hash: NameHash::from_bytes(hash),
        resource_id: ResourceId::from_bytes(resource),
        owner_public_key: key,
        version,
        issued_at,
        expires_at,
        signature,
    })
}
fn encode_delegation(value: &SignedNameDelegation) -> Result<Vec<u8>, NamingCodecError> {
    let mut out = Vec::with_capacity(246);
    header(4, &mut out);
    out.extend_from_slice(value.delegation.name_hash.as_bytes());
    out.extend_from_slice(value.delegation.parent_hash.as_bytes());
    out.extend_from_slice(value.delegation.owner.as_bytes());
    out.extend_from_slice(value.device_id.as_bytes());
    out.extend_from_slice(&value.public_key);
    out.extend_from_slice(&value.delegation.version.to_be_bytes());
    out.extend_from_slice(&value.delegation.expires_at.to_be_bytes());
    out.extend_from_slice(&value.signature);
    if out.len() > 6 + MAX_DELEGATION_LENGTH {
        return Err(NamingCodecError::InvalidLength);
    };
    Ok(out)
}
fn decode_delegation(bytes: &[u8]) -> Result<SignedNameDelegation, NamingCodecError> {
    if bytes.len() != 246 {
        return Err(NamingCodecError::InvalidLength);
    };
    validate_header(bytes, 4)?;
    let mut a = [0; 32];
    a.copy_from_slice(&bytes[6..38]);
    let mut b = [0; 32];
    b.copy_from_slice(&bytes[38..70]);
    let mut owner = [0; 32];
    owner.copy_from_slice(&bytes[70..102]);
    let mut device = [0; 32];
    device.copy_from_slice(&bytes[102..134]);
    let mut public_key = [0; 32];
    public_key.copy_from_slice(&bytes[134..166]);
    let version = u64::from_be_bytes(
        bytes[166..174]
            .try_into()
            .map_err(|_| NamingCodecError::InvalidLength)?,
    );
    let expires_at = u64::from_be_bytes(
        bytes[174..182]
            .try_into()
            .map_err(|_| NamingCodecError::InvalidLength)?,
    );
    let mut signature = [0; 64];
    signature.copy_from_slice(&bytes[182..246]);
    Ok(SignedNameDelegation {
        delegation: NameDelegation {
            name_hash: NameHash::from_bytes(a),
            parent_hash: NameHash::from_bytes(b),
            owner: crate::network::NodeId::from_bytes(owner),
            version,
            expires_at,
        },
        device_id: crate::identity::device::DeviceId::from_bytes(device),
        public_key,
        signature,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamingCodecError {
    InvalidLength,
    InvalidMagic,
    UnsupportedVersion,
    UnknownMessage,
    InvalidFlag,
    InvalidName,
    InvalidRecord,
    Name(NameError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::device::DeviceSigningKey;
    #[test]
    fn query_round_trips() {
        let q = NameQuery {
            request_id: [7; 16],
            name_hash: NameHash::of("app.aether"),
        };
        assert_eq!(NameQuery::decode(&q.encode()).unwrap(), q)
    }
    #[test]
    fn response_round_trips() {
        let k = DeviceSigningKey::generate().unwrap();
        let (_, mut r) = NameRecord::unsigned(
            "app.aether",
            ResourceId::from_bytes([9; 32]),
            k.verifying_key().to_bytes(),
            1,
            10,
            100,
        )
        .unwrap();
        r.signature = k.sign(&r.signing_bytes()).to_bytes();
        let v = NameResponse {
            request_id: [1; 16],
            name: Some("app.aether".into()),
            record: Some(r),
        };
        assert_eq!(NameResponse::decode(&v.encode().unwrap()).unwrap(), v)
    }
    #[test]
    fn announcement_rejects_hash_mismatch() {
        let k = DeviceSigningKey::generate().unwrap();
        let (_, mut r) = NameRecord::unsigned(
            "app.aether",
            ResourceId::from_bytes([1; 32]),
            k.verifying_key().to_bytes(),
            1,
            0,
            100,
        )
        .unwrap();
        r.signature = k.sign(&r.signing_bytes()).to_bytes();
        let bad = NameAnnouncement {
            name: "other.aether".into(),
            record: r,
        };
        assert!(bad.encode().is_err())
    }
}
