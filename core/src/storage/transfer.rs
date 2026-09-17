use crate::network::NodeId;
use crate::storage::chunk::ChunkId;
use crate::storage::content::ContentId;
use crate::storage::distributed::ChunkDigest;

pub const DEFAULT_UPLOAD_BYTES_PER_SECOND: u64 = 2 * 1024 * 1024;
pub const DEFAULT_DOWNLOAD_BYTES_PER_SECOND: u64 = 10 * 1024 * 1024;
pub const DEFAULT_TRANSFER_BURST_BYTES: u64 = 256 * 1024;
pub const DEFAULT_MAX_TRANSFER_PEERS: usize = 16_384;
pub const MAX_STORAGE_FRAGMENT: usize = 16 * 1024;
pub const MAX_ASSEMBLY_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    Upload,
    Download,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransferLimits {
    pub upload_bytes_per_second: u64,
    pub download_bytes_per_second: u64,
    pub burst_bytes: u64,
}
impl Default for TransferLimits {
    fn default() -> Self {
        Self {
            upload_bytes_per_second: DEFAULT_UPLOAD_BYTES_PER_SECOND,
            download_bytes_per_second: DEFAULT_DOWNLOAD_BYTES_PER_SECOND,
            burst_bytes: DEFAULT_TRANSFER_BURST_BYTES,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Bucket {
    tokens: u64,
    last_ms: u64,
}
impl Bucket {
    fn new(now: u64, burst: u64) -> Self {
        Self {
            tokens: burst,
            last_ms: now,
        }
    }
    fn replenish(&mut self, now: u64, rate: u64, burst: u64) {
        let elapsed = now.saturating_sub(self.last_ms);
        let added = rate.saturating_mul(elapsed).saturating_div(1000);
        self.tokens = self.tokens.saturating_add(added).min(burst);
        self.last_ms = now;
    }
    fn take(&mut self, amount: u64) -> bool {
        if amount > self.tokens {
            return false;
        }
        self.tokens -= amount;
        true
    }
}

#[derive(Debug, Clone, Copy)]
struct PeerBudget {
    peer: NodeId,
    upload: Bucket,
    download: Bucket,
}

pub struct TransferGovernor {
    limits: TransferLimits,
    max_peers: usize,
    peers: Vec<PeerBudget>,
}
impl TransferGovernor {
    pub fn new() -> Self {
        Self::with_limits(DEFAULT_MAX_TRANSFER_PEERS, TransferLimits::default())
    }
    pub fn with_limits(max_peers: usize, limits: TransferLimits) -> Self {
        Self {
            limits: TransferLimits {
                burst_bytes: limits.burst_bytes.max(1),
                upload_bytes_per_second: limits.upload_bytes_per_second.max(1),
                download_bytes_per_second: limits.download_bytes_per_second.max(1),
            },
            max_peers: max_peers.max(1),
            peers: Vec::new(),
        }
    }
    pub fn limits(&self) -> TransferLimits {
        self.limits
    }
    pub fn allow(
        &mut self,
        peer: NodeId,
        direction: TransferDirection,
        bytes: usize,
        now_ms: u64,
    ) -> bool {
        let index = match self.peers.iter().position(|item| item.peer == peer) {
            Some(index) => index,
            None => {
                if self.peers.len() >= self.max_peers {
                    return false;
                }
                self.peers.push(PeerBudget {
                    peer,
                    upload: Bucket::new(now_ms, self.limits.burst_bytes),
                    download: Bucket::new(now_ms, self.limits.burst_bytes),
                });
                self.peers.len() - 1
            }
        };
        let amount = bytes as u64;
        let (bucket, rate) = match direction {
            TransferDirection::Upload => (
                &mut self.peers[index].upload,
                self.limits.upload_bytes_per_second,
            ),
            TransferDirection::Download => (
                &mut self.peers[index].download,
                self.limits.download_bytes_per_second,
            ),
        };
        bucket.replenish(now_ms, rate, self.limits.burst_bytes);
        bucket.take(amount)
    }
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }
}
impl Default for TransferGovernor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransferId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkTransferRequest {
    pub transfer_id: TransferId,
    pub peer: NodeId,
    pub content: ContentId,
    pub chunk: ChunkId,
    pub offset: u32,
    pub length: u32,
    pub digest: ChunkDigest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssemblyError {
    InvalidRange,
    CapacityExceeded,
    ConflictingData,
    IntegrityFailure,
}

pub struct ChunkAssembler {
    content: ContentId,
    chunk: ChunkId,
    expected_digest: ChunkDigest,
    expected_len: usize,
    bytes: Vec<u8>,
    received: Vec<u8>,
    received_bytes: usize,
}
impl ChunkAssembler {
    pub fn new(
        content: ContentId,
        chunk: ChunkId,
        expected_len: usize,
        digest: ChunkDigest,
    ) -> Result<Self, AssemblyError> {
        if expected_len > MAX_ASSEMBLY_BYTES {
            return Err(AssemblyError::CapacityExceeded);
        }
        let bitmap_len = expected_len.saturating_add(7) / 8;
        Ok(Self {
            content,
            chunk,
            expected_digest: digest,
            expected_len,
            bytes: vec![0; expected_len],
            received: vec![0; bitmap_len],
            received_bytes: 0,
        })
    }
    pub fn content(&self) -> ContentId {
        self.content
    }
    pub fn chunk(&self) -> ChunkId {
        self.chunk
    }
    pub fn expected_len(&self) -> usize {
        self.expected_len
    }
    pub fn received_bytes(&self) -> usize {
        self.received_bytes
    }
    pub fn complete(&self) -> bool {
        self.received_bytes == self.expected_len
    }

    pub fn write(&mut self, offset: usize, payload: &[u8]) -> Result<(), AssemblyError> {
        let end = offset
            .checked_add(payload.len())
            .ok_or(AssemblyError::InvalidRange)?;
        if end > self.expected_len {
            return Err(AssemblyError::InvalidRange);
        }
        for (index, value) in payload.iter().enumerate() {
            let position = offset + index;
            let mask = 1u8 << (position & 7);
            let slot = position >> 3;
            if self.received[slot] & mask != 0 {
                if self.bytes[position] != *value {
                    return Err(AssemblyError::ConflictingData);
                }
            } else {
                self.bytes[position] = *value;
                self.received[slot] |= mask;
                self.received_bytes += 1;
            }
        }
        Ok(())
    }

    pub fn finish(self) -> Result<Vec<u8>, AssemblyError> {
        if !self.complete() {
            return Err(AssemblyError::InvalidRange);
        }
        if ChunkDigest::of(self.content, self.chunk, &self.bytes) != self.expected_digest {
            return Err(AssemblyError::IntegrityFailure);
        }
        Ok(self.bytes)
    }
}

#[derive(Debug, Clone)]
pub struct PendingTransfer {
    pub request: ChunkTransferRequest,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
}

pub struct TransferQueue {
    max_pending: usize,
    next_id: u64,
    pending: Vec<PendingTransfer>,
}
impl TransferQueue {
    pub fn new() -> Self {
        Self::with_limit(4096)
    }
    pub fn with_limit(max_pending: usize) -> Self {
        Self {
            max_pending: max_pending.max(1),
            next_id: 1,
            pending: Vec::new(),
        }
    }
    pub fn enqueue(
        &mut self,
        mut request: ChunkTransferRequest,
        now_ms: u64,
        ttl_ms: u64,
    ) -> Option<TransferId> {
        if self.pending.len() >= self.max_pending {
            return None;
        }
        let id = TransferId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1).max(1);
        request.transfer_id = id;
        self.pending.push(PendingTransfer {
            request,
            created_at_ms: now_ms,
            expires_at_ms: now_ms.saturating_add(ttl_ms.max(1)),
        });
        Some(id)
    }
    pub fn remove_expired(&mut self, now_ms: u64) -> usize {
        let before = self.pending.len();
        self.pending.retain(|item| now_ms < item.expires_at_ms);
        before.saturating_sub(self.pending.len())
    }
    pub fn len(&self) -> usize {
        self.pending.len()
    }
}
impl Default for TransferQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node() -> NodeId {
        NodeId::from_bytes([5; 32])
    }
    fn content() -> ContentId {
        ContentId::from_bytes(b"transfer-test")
    }

    #[test]
    fn governor_enforces_burst_budget() {
        let mut governor = TransferGovernor::with_limits(
            4,
            TransferLimits {
                upload_bytes_per_second: 100,
                download_bytes_per_second: 100,
                burst_bytes: 100,
            },
        );
        assert!(governor.allow(node(), TransferDirection::Upload, 100, 0));
        assert!(!governor.allow(node(), TransferDirection::Upload, 1, 0));
        assert!(governor.allow(node(), TransferDirection::Upload, 10, 100));
    }

    #[test]
    fn assembler_supports_out_of_order_fragments() {
        let data = vec![3u8; 1024];
        let digest = ChunkDigest::of(content(), ChunkId(0), &data);
        let mut assembler = ChunkAssembler::new(content(), ChunkId(0), data.len(), digest).unwrap();
        assembler.write(512, &data[512..]).unwrap();
        assembler.write(0, &data[..512]).unwrap();
        assert_eq!(assembler.finish().unwrap(), data);
    }
}
