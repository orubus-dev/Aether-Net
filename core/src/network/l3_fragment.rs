use super::{AetherFlowId, AetherL3Packet, L3Error, L3Flags, MAX_L3_PAYLOAD};

const FRAGMENT_META_LEN: usize = 4;
pub const DEFAULT_MAX_REASSEMBLIES: usize = 1024;
pub const DEFAULT_MAX_REASSEMBLY_BYTES: usize = 8 * 1024 * 1024;
pub const DEFAULT_REASSEMBLY_TIMEOUT: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentError {
    InvalidLimit,
    TooManyFragments,
    PayloadTooLarge,
    InvalidMetadata,
    ConflictingFragment,
    ReassemblyTableFull,
    ReassemblyBytesExceeded,
}

/// Split one native L3 packet into bounded fragments. The flow identifier is
/// preserved across all fragments and a compact fragment index/count prefix
/// is carried inside the L3 payload.
pub fn fragment_packet(
    packet: &AetherL3Packet,
    max_fragment_payload: usize,
) -> Result<Vec<AetherL3Packet>, FragmentError> {
    if max_fragment_payload <= FRAGMENT_META_LEN {
        return Err(FragmentError::InvalidLimit);
    }
    if packet.payload.len() <= max_fragment_payload {
        return Ok(vec![packet.clone()]);
    }

    let chunk_size = max_fragment_payload - FRAGMENT_META_LEN;
    let count = packet.payload.len().div_ceil(chunk_size);
    if count > u16::MAX as usize {
        return Err(FragmentError::TooManyFragments);
    }

    let mut fragments = Vec::with_capacity(count);
    for index in 0..count {
        let start = index * chunk_size;
        let end = (start + chunk_size).min(packet.payload.len());
        let mut payload = Vec::with_capacity(FRAGMENT_META_LEN + end - start);
        payload.extend_from_slice(&(index as u16).to_be_bytes());
        payload.extend_from_slice(&(count as u16).to_be_bytes());
        payload.extend_from_slice(&packet.payload[start..end]);

        let mut fragment = packet.clone();
        fragment.flags = L3Flags(packet.flags.0 | L3Flags::FRAGMENT.0);
        if index + 1 == count {
            fragment.flags = L3Flags(fragment.flags.0 | L3Flags::FINAL_FRAGMENT.0);
        } else {
            fragment.flags = L3Flags(fragment.flags.0 & !L3Flags::FINAL_FRAGMENT.0);
        }
        fragment.payload = payload;
        if fragment.payload.len() > MAX_L3_PAYLOAD {
            return Err(FragmentError::PayloadTooLarge);
        }
        fragments.push(fragment);
    }

    Ok(fragments)
}

struct ReassemblyEntry {
    flow_id: AetherFlowId,
    source: super::AetherAddress,
    destination: super::AetherAddress,
    priority: super::TrafficPriority,
    sequence: u64,
    hop_limit: u8,
    flags: L3Flags,
    fragment_count: u16,
    received: Vec<Option<Vec<u8>>>,
    bytes: usize,
    deadline: u64,
}

impl ReassemblyEntry {
    fn new(packet: &AetherL3Packet, now: u64, timeout: u64) -> Result<Self, FragmentError> {
        let (index, count, payload) = decode_metadata(packet)?;
        let mut received = vec![None; count as usize];
        received[index as usize] = Some(payload.to_vec());
        Ok(Self {
            flow_id: packet.flow_id,
            source: packet.source,
            destination: packet.destination,
            priority: packet.priority,
            sequence: packet.sequence,
            hop_limit: packet.hop_limit,
            flags: packet.flags,
            fragment_count: count,
            received,
            bytes: payload.len(),
            deadline: now.saturating_add(timeout),
        })
    }

    fn accepts(&self, packet: &AetherL3Packet) -> bool {
        self.source == packet.source
            && self.destination == packet.destination
            && self.priority == packet.priority
            && self.sequence == packet.sequence
            && self.fragment_count > 0
    }

    fn push(&mut self, packet: &AetherL3Packet) -> Result<Option<Vec<u8>>, FragmentError> {
        if !self.accepts(packet) {
            return Err(FragmentError::ConflictingFragment);
        }

        let (index, count, payload) = decode_metadata(packet)?;
        if count != self.fragment_count || index as usize >= self.received.len() {
            return Err(FragmentError::ConflictingFragment);
        }

        match &self.received[index as usize] {
            Some(existing) if existing.as_slice() == payload => return Ok(None),
            Some(_) => return Err(FragmentError::ConflictingFragment),
            None => {}
        }

        self.bytes += payload.len();
        self.received[index as usize] = Some(payload.to_vec());

        if self.received.iter().all(Option::is_some) {
            let total = self.received.iter().flatten().map(Vec::len).sum();
            let mut output = Vec::with_capacity(total);
            for fragment in &self.received {
                output.extend_from_slice(fragment.as_deref().expect("checked above"));
            }
            return Ok(Some(output));
        }

        Ok(None)
    }
}

fn decode_metadata(packet: &AetherL3Packet) -> Result<(u16, u16, &[u8]), FragmentError> {
    if !packet.flags.contains(L3Flags::FRAGMENT) || packet.payload.len() < FRAGMENT_META_LEN {
        return Err(FragmentError::InvalidMetadata);
    }

    let index = u16::from_be_bytes([packet.payload[0], packet.payload[1]]);
    let count = u16::from_be_bytes([packet.payload[2], packet.payload[3]]);
    if count == 0 || index >= count {
        return Err(FragmentError::InvalidMetadata);
    }

    let final_fragment = packet.flags.contains(L3Flags::FINAL_FRAGMENT);
    if final_fragment != (index + 1 == count) {
        return Err(FragmentError::InvalidMetadata);
    }

    Ok((index, count, &packet.payload[FRAGMENT_META_LEN..]))
}

pub struct L3ReassemblyTable {
    max_entries: usize,
    max_bytes: usize,
    timeout: u64,
    bytes: usize,
    entries: Vec<ReassemblyEntry>,
}

impl L3ReassemblyTable {
    pub fn new() -> Self {
        Self::with_limits(
            DEFAULT_MAX_REASSEMBLIES,
            DEFAULT_MAX_REASSEMBLY_BYTES,
            DEFAULT_REASSEMBLY_TIMEOUT,
        )
    }

    pub fn with_limits(max_entries: usize, max_bytes: usize, timeout: u64) -> Self {
        Self {
            max_entries: max_entries.max(1),
            max_bytes: max_bytes.max(1),
            timeout,
            bytes: 0,
            entries: Vec::new(),
        }
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn buffered_bytes(&self) -> usize {
        self.bytes
    }

    pub fn expire(&mut self, now: u64) -> usize {
        let mut removed_bytes = 0;
        self.entries.retain(|entry| {
            let keep = now < entry.deadline;
            if !keep {
                removed_bytes += entry.bytes;
            }
            keep
        });
        self.bytes -= removed_bytes;
        self.entries.len()
    }

    pub fn push(
        &mut self,
        packet: &AetherL3Packet,
        now: u64,
    ) -> Result<Option<AetherL3Packet>, FragmentError> {
        if !packet.flags.contains(L3Flags::FRAGMENT) {
            return Ok(Some(packet.clone()));
        }

        let (_, _, payload) = decode_metadata(packet)?;
        if self.bytes.saturating_add(payload.len()) > self.max_bytes {
            return Err(FragmentError::ReassemblyBytesExceeded);
        }

        let existing_index = self
            .entries
            .iter()
            .position(|entry| entry.flow_id == packet.flow_id);

        if let Some(index) = existing_index {
            let result = self.entries[index].push(packet)?;
            self.bytes = self.entries.iter().map(|entry| entry.bytes).sum();
            if let Some(payload) = result {
                let entry = self.entries.swap_remove(index);
                self.bytes = self.entries.iter().map(|candidate| candidate.bytes).sum();
                let mut reconstructed = packet.clone();
                reconstructed.flags = L3Flags(entry.flags.0 & !L3Flags::FRAGMENT.0 & !L3Flags::FINAL_FRAGMENT.0);
                reconstructed.payload = payload;
                return Ok(Some(reconstructed));
            }
            return Ok(None);
        }

        if self.entries.len() >= self.max_entries {
            return Err(FragmentError::ReassemblyTableFull);
        }

        let entry = ReassemblyEntry::new(packet, now, self.timeout)?;
        self.bytes += entry.bytes;
        self.entries.push(entry);
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::{AetherAddress, AETHER_ADDRESS_LENGTH, TrafficPriority};

    fn address(seed: u8) -> AetherAddress {
        AetherAddress::from_bytes([seed; AETHER_ADDRESS_LENGTH])
    }

    #[test]
    fn fragments_round_trip_through_reassembly() {
        let original = AetherL3Packet::new(
            address(1),
            address(2),
            TrafficPriority::Interactive,
            L3Flags::RELAYABLE,
            16,
            (0u8..=99).collect(),
        )
        .unwrap();

        let fragments = fragment_packet(&original, 24).unwrap();
        assert!(fragments.len() > 1);

        let mut table = L3ReassemblyTable::with_limits(8, 1024, 30);
        let mut rebuilt = None;
        for fragment in fragments.into_iter().rev() {
            rebuilt = table.push(&fragment, 10).unwrap().or(rebuilt);
        }

        let rebuilt = rebuilt.unwrap();
        assert_eq!(rebuilt.source, original.source);
        assert_eq!(rebuilt.destination, original.destination);
        assert_eq!(rebuilt.payload, original.payload);
        assert!(!rebuilt.flags.contains(L3Flags::FRAGMENT));
        assert_eq!(table.entry_count(), 0);
    }

    #[test]
    fn conflicting_duplicate_fragment_is_rejected() {
        let original = AetherL3Packet::new(
            address(1),
            address(2),
            TrafficPriority::Interactive,
            L3Flags::RELAYABLE,
            16,
            vec![7; 80],
        )
        .unwrap();
        let mut fragments = fragment_packet(&original, 20).unwrap();
        let first = fragments.remove(0);
        let mut conflict = first.clone();
        *conflict.payload.last_mut().unwrap() ^= 0xff;

        let mut table = L3ReassemblyTable::new();
        table.push(&first, 1).unwrap();
        assert_eq!(
            table.push(&conflict, 1).unwrap_err(),
            FragmentError::ConflictingFragment
        );
    }
}
