use crate::network::{NodeId, TrafficPriority};
use crate::route::{RouteMetrics, RoutePath};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

pub const DEFAULT_MAX_NODES: usize = 65_536;
pub const DEFAULT_MAX_LINKS: usize = 262_144;
pub const DEFAULT_MAX_ADVERTISEMENT_NEIGHBORS: usize = 256;
pub const DEFAULT_MAX_PATHS: usize = 8;
pub const DEFAULT_MAX_SEARCH_STATES: usize = 100_000;
pub const DEFAULT_ROUTE_TTL_SECONDS: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteNeighbor {
    pub node_id: NodeId,
    pub metrics: RouteMetrics,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteAdvertisement {
    pub origin: NodeId,
    pub sequence: u64,
    pub issued_at: u64,
    pub expires_at: u64,
    pub neighbors: Vec<RouteNeighbor>,
}
impl RouteAdvertisement {
    pub fn new(origin: NodeId, sequence: u64, now: u64, neighbors: Vec<RouteNeighbor>) -> Self {
        Self {
            origin,
            sequence,
            issued_at: now,
            expires_at: now.saturating_add(DEFAULT_ROUTE_TTL_SECONDS),
            neighbors,
        }
    }
    pub fn with_expiry(
        origin: NodeId,
        sequence: u64,
        issued_at: u64,
        expires_at: u64,
        neighbors: Vec<RouteNeighbor>,
    ) -> Self {
        Self {
            origin,
            sequence,
            issued_at,
            expires_at,
            neighbors,
        }
    }
    pub fn is_valid_at(&self, now: u64) -> bool {
        now < self.expires_at && self.expires_at >= self.issued_at
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingError {
    NodeTableFull,
    LinkTableFull,
    InvalidAdvertisement,
    InvalidMetrics,
    NoRoute,
    SearchBudgetExceeded,
}
#[derive(Debug, Clone, Copy, PartialEq)]
struct SearchState {
    score: f64,
    node: NodeId,
    path_index: usize,
}
impl Eq for SearchState {}
impl Ord for SearchState {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score
            .total_cmp(&other.score)
            .then_with(|| self.node.as_bytes().cmp(other.node.as_bytes()))
            .then_with(|| self.path_index.cmp(&other.path_index))
    }
}
impl PartialOrd for SearchState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Copy)]
struct Edge {
    to: NodeId,
    metrics: RouteMetrics,
}
struct Candidate {
    cost: f64,
    node: NodeId,
    first_hop: NodeId,
}
impl Eq for Candidate {}
impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.cost.total_cmp(&other.cost) == Ordering::Equal
            && self.node == other.node
            && self.first_hop == other.first_hop
    }
}
impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .total_cmp(&self.cost)
            .then_with(|| self.node.as_bytes().cmp(other.node.as_bytes()))
            .then_with(|| self.first_hop.as_bytes().cmp(other.first_hop.as_bytes()))
    }
}
impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct DistributedRoutingEngine {
    max_nodes: usize,
    max_links: usize,
    max_paths: usize,
    max_search_states: usize,
    nodes: HashSet<NodeId>,
    edges: HashMap<NodeId, Vec<Edge>>,
}
impl DistributedRoutingEngine {
    pub fn new() -> Self {
        Self::with_limits(
            DEFAULT_MAX_NODES,
            DEFAULT_MAX_LINKS,
            DEFAULT_MAX_PATHS,
            DEFAULT_MAX_SEARCH_STATES,
        )
    }
    pub fn with_limits(
        max_nodes: usize,
        max_links: usize,
        max_paths: usize,
        max_search_states: usize,
    ) -> Self {
        Self {
            max_nodes: max_nodes.max(1),
            max_links: max_links.max(1),
            max_paths: max_paths.max(1),
            max_search_states: max_search_states.max(1),
            nodes: HashSet::new(),
            edges: HashMap::new(),
        }
    }
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    pub fn link_count(&self) -> usize {
        self.edges.values().map(Vec::len).sum()
    }
    pub fn install_advertisement(
        &mut self,
        ad: RouteAdvertisement,
        now: u64,
    ) -> Result<(), RoutingError> {
        if ad.origin == NodeId::from_bytes([0; 32])
            || !ad.is_valid_at(now)
            || ad.neighbors.len() > DEFAULT_MAX_ADVERTISEMENT_NEIGHBORS
        {
            return Err(RoutingError::InvalidAdvertisement);
        }
        if !self.nodes.contains(&ad.origin) && self.nodes.len() >= self.max_nodes {
            return Err(RoutingError::NodeTableFull);
        }
        self.nodes.insert(ad.origin);
        let existing = self.edges.entry(ad.origin).or_default();
        existing.clear();
        for n in ad.neighbors {
            if n.node_id == NodeId::from_bytes([0; 32]) {
                return Err(RoutingError::InvalidAdvertisement);
            }
            if existing.len() >= DEFAULT_MAX_ADVERTISEMENT_NEIGHBORS {
                return Err(RoutingError::LinkTableFull);
            }
            existing.push(Edge {
                to: n.node_id,
                metrics: n.metrics,
            });
            self.nodes.insert(n.node_id);
        }
        Ok(())
    }
    pub fn remove_node(&mut self, node: NodeId) -> bool {
        let mut removed = self.nodes.remove(&node);
        removed |= self.edges.remove(&node).is_some();
        for edges in self.edges.values_mut() {
            let before = edges.len();
            edges.retain(|edge| edge.to != node);
            removed |= before != edges.len();
        }
        removed
    }
    pub fn best_paths(
        &self,
        source: NodeId,
        destination: NodeId,
        priority: TrafficPriority,
    ) -> Result<Vec<RoutePath>, RoutingError> {
        if source == destination {
            return Ok(Vec::new());
        }
        if !self.nodes.contains(&source) || !self.nodes.contains(&destination) {
            return Err(RoutingError::NoRoute);
        }
        let mut heap = BinaryHeap::new();
        let mut best: HashMap<NodeId, f64> = HashMap::new();
        let mut previous: HashMap<NodeId, (NodeId, RouteMetrics)> = HashMap::new();
        let mut states = 0usize;
        heap.push(Candidate {
            cost: 0.0,
            node: source,
            first_hop: source,
        });
        best.insert(source, 0.0);
        while let Some(current) = heap.pop() {
            states += 1;
            if states > self.max_search_states {
                return Err(RoutingError::SearchBudgetExceeded);
            }
            if current.node == destination {
                break;
            }
            if current.cost > best.get(&current.node).copied().unwrap_or(f64::INFINITY) {
                continue;
            }
            for edge in self.edges.get(&current.node).into_iter().flatten() {
                let metric = edge.metrics;
                let step = metric.cost(priority);
                let next = current.cost + step;
                if next < best.get(&edge.to).copied().unwrap_or(f64::INFINITY) {
                    best.insert(edge.to, next);
                    previous.insert(edge.to, (current.node, metric));
                    heap.push(Candidate {
                        cost: next,
                        node: edge.to,
                        first_hop: if current.node == source {
                            edge.to
                        } else {
                            current.first_hop
                        },
                    });
                }
            }
        }
        if !best.contains_key(&destination) {
            return Err(RoutingError::NoRoute);
        }
        let mut hops = Vec::new();
        let mut node = destination;
        while node != source {
            let Some((prev, metrics)) = previous.get(&node).copied() else {
                return Err(RoutingError::NoRoute);
            };
            hops.push((prev, node, metrics));
            node = prev;
        }
        hops.reverse();
        let nodes: Vec<NodeId> = hops.iter().map(|(_, to, _)| *to).collect();
        let mut path = RoutePath::new(source, nodes.clone());
        if let Some((_, _, metrics)) = hops.first() {
            path.metrics = *metrics;
        }
        Ok(vec![path])
    }
}
impl Default for DistributedRoutingEngine {
    fn default() -> Self {
        Self::new()
    }
}
