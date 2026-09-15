use crate::network::NodeId;
use crate::services::ServiceId;
use sha2::{Digest, Sha256};

pub const DEFAULT_MAX_WORKLOADS: usize = 16_384;
pub const DEFAULT_MAX_NODES: usize = 16_384;
pub const DEFAULT_MAX_PLACEMENTS: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkloadId([u8; 32]);
impl WorkloadId {
    pub fn derive(owner: NodeId, nonce: u64) -> Self {
        let mut h = Sha256::new();
        h.update(b"aether:workload:v1:");
        h.update(owner.as_bytes());
        h.update(nonce.to_be_bytes());
        Self(h.finalize().into())
    }
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComputeCapacity {
    pub cpu_millis: u32,
    pub memory_bytes: u64,
    pub storage_bytes: u64,
    pub bandwidth_bps: u64,
}
impl ComputeCapacity {
    pub fn fits(&self, req: &Self) -> bool {
        self.cpu_millis >= req.cpu_millis
            && self.memory_bytes >= req.memory_bytes
            && self.storage_bytes >= req.storage_bytes
            && self.bandwidth_bps >= req.bandwidth_bps
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComputeNode {
    pub node_id: NodeId,
    pub capacity: ComputeCapacity,
    pub available: ComputeCapacity,
    pub reliability_ppm: u32,
    pub cost_micros: u64,
    pub expires_at: u64,
}
impl ComputeNode {
    pub fn is_valid_at(&self, now: u64) -> bool {
        now < self.expires_at
            && self.available.fits(&self.capacity)
            && self.reliability_ppm <= 1_000_000
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkloadSpec {
    pub service: ServiceId,
    pub requirements: ComputeCapacity,
    pub replicas: u16,
    pub migratable: bool,
    pub priority: u8,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Workload {
    pub id: WorkloadId,
    pub owner: NodeId,
    pub spec: WorkloadSpec,
    pub version: u64,
    pub expires_at: u64,
}
impl Workload {
    pub fn is_valid_at(&self, now: u64) -> bool {
        now < self.expires_at
            && self.spec.replicas.max(1) <= 1024
            && self.spec.requirements.cpu_millis > 0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub workload: WorkloadId,
    pub node: NodeId,
    pub version: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputeError {
    NodeCapacityExceeded,
    WorkloadCapacityExceeded,
    PlacementCapacityExceeded,
    InvalidNode,
    InvalidWorkload,
    NoPlacement,
    MigrationNotAllowed,
}
pub struct DistributedCompute {
    max_nodes: usize,
    max_workloads: usize,
    max_placements: usize,
    nodes: Vec<ComputeNode>,
    workloads: Vec<Workload>,
    placements: Vec<Placement>,
}
impl DistributedCompute {
    pub fn new() -> Self {
        Self::with_limits(
            DEFAULT_MAX_NODES,
            DEFAULT_MAX_WORKLOADS,
            DEFAULT_MAX_PLACEMENTS,
        )
    }
    pub fn with_limits(max_nodes: usize, max_workloads: usize, max_placements: usize) -> Self {
        Self {
            max_nodes: max_nodes.max(1),
            max_workloads: max_workloads.max(1),
            max_placements: max_placements.max(1),
            nodes: Vec::new(),
            workloads: Vec::new(),
            placements: Vec::new(),
        }
    }
    pub fn register_node(&mut self, node: ComputeNode, now: u64) -> Result<bool, ComputeError> {
        if !node.is_valid_at(now) {
            return Err(ComputeError::InvalidNode);
        }
        if let Some(existing) = self.nodes.iter_mut().find(|n| n.node_id == node.node_id) {
            *existing = node;
            return Ok(true);
        }
        if self.nodes.len() >= self.max_nodes {
            return Err(ComputeError::NodeCapacityExceeded);
        }
        self.nodes.push(node);
        Ok(true)
    }
    pub fn submit(&mut self, workload: Workload, now: u64) -> Result<bool, ComputeError> {
        if !workload.is_valid_at(now) {
            return Err(ComputeError::InvalidWorkload);
        }
        if let Some(existing) = self.workloads.iter_mut().find(|w| w.id == workload.id) {
            if workload.version < existing.version {
                return Ok(false);
            }
            *existing = workload;
            self.placements.retain(|p| p.workload != workload.id);
            return Ok(true);
        }
        if self.workloads.len() >= self.max_workloads {
            return Err(ComputeError::WorkloadCapacityExceeded);
        }
        self.workloads.push(workload);
        Ok(true)
    }
    pub fn place(&mut self, id: WorkloadId, now: u64) -> Result<Vec<NodeId>, ComputeError> {
        let w = *self
            .workloads
            .iter()
            .find(|w| w.id == id && w.is_valid_at(now))
            .ok_or(ComputeError::InvalidWorkload)?;
        let need = w.spec.replicas.max(1) as usize;
        let mut c: Vec<&ComputeNode> = self
            .nodes
            .iter()
            .filter(|n| n.is_valid_at(now) && n.available.fits(&w.spec.requirements))
            .collect();
        c.sort_by(|a, b| {
            node_score(b)
                .cmp(&node_score(a))
                .then_with(|| a.node_id.as_bytes().cmp(b.node_id.as_bytes()))
        });
        if c.len() < need {
            return Err(ComputeError::NoPlacement);
        }
        let selected = c
            .into_iter()
            .take(need)
            .map(|n| n.node_id)
            .collect::<Vec<_>>();
        let current = self.placements.iter().filter(|p| p.workload == id).count();
        if self
            .placements
            .len()
            .saturating_sub(current)
            .saturating_add(selected.len())
            > self.max_placements
        {
            return Err(ComputeError::PlacementCapacityExceeded);
        }
        self.placements.retain(|p| p.workload != id);
        for node in &selected {
            self.placements.push(Placement {
                workload: id,
                node: *node,
                version: w.version,
            })
        }
        Ok(selected)
    }
    pub fn migrate(
        &mut self,
        id: WorkloadId,
        from: NodeId,
        to: NodeId,
        now: u64,
    ) -> Result<bool, ComputeError> {
        let w = *self
            .workloads
            .iter()
            .find(|w| w.id == id && w.is_valid_at(now))
            .ok_or(ComputeError::InvalidWorkload)?;
        if !w.spec.migratable {
            return Err(ComputeError::MigrationNotAllowed);
        }
        let target = *self
            .nodes
            .iter()
            .find(|n| n.node_id == to && n.is_valid_at(now))
            .ok_or(ComputeError::NoPlacement)?;
        if !target.available.fits(&w.spec.requirements) {
            return Err(ComputeError::NoPlacement);
        }
        if let Some(p) = self
            .placements
            .iter_mut()
            .find(|p| p.workload == id && p.node == from)
        {
            p.node = to;
            p.version = w.version;
            return Ok(true);
        }
        Err(ComputeError::NoPlacement)
    }
    pub fn placements(&self, id: WorkloadId) -> impl Iterator<Item = &Placement> {
        self.placements.iter().filter(move |p| p.workload == id)
    }
    pub fn expire(&mut self, now: u64) -> usize {
        let before = self.workloads.len() + self.nodes.len() + self.placements.len();
        self.workloads.retain(|w| w.is_valid_at(now));
        self.nodes.retain(|n| n.is_valid_at(now));
        self.placements
            .retain(|p| self.workloads.iter().any(|w| w.id == p.workload));
        let after = self.workloads.len() + self.nodes.len() + self.placements.len();
        before.saturating_sub(after)
    }
}
impl Default for DistributedCompute {
    fn default() -> Self {
        Self::new()
    }
}
fn node_score(n: &ComputeNode) -> u64 {
    let capacity = n.available.cpu_millis as u64
        + n.available.memory_bytes / 1_048_576
        + n.available.storage_bytes / 1_073_741_824
        + n.available.bandwidth_bps / 1_000_000;
    capacity
        .saturating_add(n.reliability_ppm as u64)
        .saturating_sub(n.cost_micros.min(1_000_000))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn node(x: u8) -> NodeId {
        NodeId::from_bytes([x; 32])
    }
    fn service(x: u8) -> ServiceId {
        ServiceId([x; 16])
    }
    fn cap() -> ComputeCapacity {
        ComputeCapacity {
            cpu_millis: 1000,
            memory_bytes: 1024 * 1024 * 1024,
            storage_bytes: 10 * 1024 * 1024 * 1024,
            bandwidth_bps: 10_000_000,
        }
    }
    fn n(x: u8) -> ComputeNode {
        ComputeNode {
            node_id: node(x),
            capacity: cap(),
            available: cap(),
            reliability_ppm: 990_000,
            cost_micros: x as u64,
            expires_at: 100,
        }
    }
    fn w() -> Workload {
        Workload {
            id: WorkloadId::derive(node(9), 1),
            owner: node(9),
            spec: WorkloadSpec {
                service: service(1),
                requirements: ComputeCapacity {
                    cpu_millis: 100,
                    memory_bytes: 128 * 1024 * 1024,
                    storage_bytes: 1024,
                    bandwidth_bps: 1_000_000,
                },
                replicas: 2,
                migratable: true,
                priority: 1,
            },
            version: 1,
            expires_at: 100,
        }
    }
    #[test]
    fn places_requested_replicas() {
        let mut c = DistributedCompute::with_limits(4, 4, 8);
        c.register_node(n(1), 1).unwrap();
        c.register_node(n(2), 1).unwrap();
        c.register_node(n(3), 1).unwrap();
        let id = w().id;
        c.submit(w(), 1).unwrap();
        assert_eq!(c.place(id, 1).unwrap().len(), 2);
        assert_eq!(c.placements(id).count(), 2)
    }
    #[test]
    fn rejects_insufficient_capacity() {
        let mut c = DistributedCompute::new();
        c.register_node(n(1), 1).unwrap();
        let mut work = w();
        work.spec.requirements.cpu_millis = 2000;
        c.submit(work, 1).unwrap();
        assert_eq!(c.place(work.id, 1), Err(ComputeError::NoPlacement));
    }
    #[test]
    fn versioned_workload_rejects_older_update() {
        let mut c = DistributedCompute::new();
        let mut a = w();
        a.version = 4;
        let mut b = w();
        b.version = 3;
        c.submit(a, 1).unwrap();
        assert!(!c.submit(b, 1).unwrap());
    }
    #[test]
    fn migration_requires_policy() {
        let mut c = DistributedCompute::new();
        c.register_node(n(1), 1).unwrap();
        c.register_node(n(2), 1).unwrap();
        let mut work = w();
        work.spec.replicas = 1;
        work.spec.migratable = false;
        c.submit(work, 1).unwrap();
        c.place(work.id, 1).unwrap();
        assert_eq!(
            c.migrate(work.id, node(1), node(2), 1),
            Err(ComputeError::MigrationNotAllowed)
        );
    }
}
