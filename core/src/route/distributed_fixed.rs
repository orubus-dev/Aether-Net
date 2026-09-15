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

#[derive(Debug, Clone)]
pub struct RouteAdvertisement {
    pub origin: NodeId,
    pub sequence: u64,
    pub issued_at: u64,
    pub expires_at: u64,
    pub neighbors: Vec<RouteNeighbor>,
}

impl RouteAdvertisement {
    pub fn new(origin: NodeId, sequence: u64, now: u64, neighbors: Vec<RouteNeighbor>) -> Self {
        Self { origin, sequence, issued_at: now, expires_at: now.saturating_add(DEFAULT_ROUTE_TTL_SECONDS), neighbors }
    }
    pub fn with_expiry(origin: NodeId, sequence: u64, issued_at: u64, expires_at: u64, neighbors: Vec<RouteNeighbor>) -> Self {
        Self { origin, sequence, issued_at, expires_at, neighbors }
    }
    pub fn is_valid_at(&self, now: u64) -> bool { now < self.expires_at && self.expires_at >= self.issued_at }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingError { NodeTableFull, LinkTableFull, InvalidAdvertisement, InvalidMetrics, NoRoute, SearchBudgetExceeded }

#[derive(Debug, Clone, Copy, PartialEq)]
struct SearchState { score: f64, node: NodeId, path_index: usize }
impl Eq for SearchState {}
impl Ord for SearchState {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score.total_cmp(&other.score)
            .then_with(|| self.node.as_bytes().cmp(other.node.as_bytes()))
            .then_with(|| self.path_index.cmp(&other.path_index))
    }
}
impl PartialOrd for SearchState { fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) } }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TopologyLink { from: NodeId, to: NodeId, metrics: RouteMetrics, expires_at: u64 }

pub struct DistributedRoutingEngine {
    local_node: NodeId,
    max_nodes: usize,
    max_links: usize,
    max_advertisement_neighbors: usize,
    max_paths: usize,
    max_search_states: usize,
    links: Vec<TopologyLink>,
    known_sequences: HashMap<NodeId, u64>,
}

impl DistributedRoutingEngine {
    pub fn new(local_node: NodeId) -> Self { Self::with_limits(local_node, DEFAULT_MAX_NODES, DEFAULT_MAX_LINKS, DEFAULT_MAX_ADVERTISEMENT_NEIGHBORS, DEFAULT_MAX_PATHS, DEFAULT_MAX_SEARCH_STATES) }
    pub fn with_limits(local_node: NodeId, max_nodes: usize, max_links: usize, max_advertisement_neighbors: usize, max_paths: usize, max_search_states: usize) -> Self {
        Self { local_node, max_nodes: max_nodes.max(1), max_links: max_links.max(1), max_advertisement_neighbors: max_advertisement_neighbors.max(1), max_paths: max_paths.max(1), max_search_states: max_search_states.max(1), links: Vec::new(), known_sequences: HashMap::new() }
    }
    pub fn local_node(&self) -> NodeId { self.local_node }
    pub fn link_count(&self) -> usize { self.links.len() }
    pub fn node_count(&self) -> usize { let mut nodes = HashSet::new(); nodes.insert(self.local_node); for link in &self.links { nodes.insert(link.from); nodes.insert(link.to); } nodes.len() }
    pub fn install_advertisement(&mut self, ad: RouteAdvertisement, now: u64) -> Result<bool, RoutingError> {
        if !ad.is_valid_at(now) || ad.expires_at <= ad.issued_at || ad.neighbors.len() > self.max_advertisement_neighbors { return Err(RoutingError::InvalidAdvertisement); }
        if ad.neighbors.iter().any(|neighbor| neighbor.node_id == ad.origin || !metrics_valid(&neighbor.metrics)) { return Err(RoutingError::InvalidMetrics); }
        if let Some(previous) = self.known_sequences.get(&ad.origin) { if ad.sequence <= *previous { return Ok(false); } } else if self.node_count() >= self.max_nodes { return Err(RoutingError::NodeTableFull); }
        let retained = self.links.iter().filter(|link| link.from != ad.origin).count();
        if retained.saturating_add(ad.neighbors.len()) > self.max_links { return Err(RoutingError::LinkTableFull); }
        self.links.retain(|link| link.from != ad.origin);
        for neighbor in ad.neighbors { self.links.push(TopologyLink { from: ad.origin, to: neighbor.node_id, metrics: neighbor.metrics, expires_at: ad.expires_at }); }
        self.known_sequences.insert(ad.origin, ad.sequence);
        Ok(true)
    }
    pub fn remove_origin(&mut self, origin: NodeId) -> bool { let before = self.links.len(); self.links.retain(|link| link.from != origin); self.known_sequences.remove(&origin); before != self.links.len() }
    pub fn expire(&mut self, now: u64) -> usize { let before = self.links.len(); self.links.retain(|link| now < link.expires_at); before - self.links.len() }
    pub fn advertisement_for(&self, origin: NodeId, sequence: u64, now: u64) -> RouteAdvertisement {
        let neighbors = self.links.iter().filter(|link| link.from == origin && now < link.expires_at).map(|link| RouteNeighbor { node_id: link.to, metrics: link.metrics }).take(self.max_advertisement_neighbors).collect();
        RouteAdvertisement::new(origin, sequence, now, neighbors)
    }
    pub fn best_route(&self, destination: NodeId, priority: TrafficPriority, now: u64) -> Result<RoutePath, RoutingError> {
        self.routes(destination, priority, now, 1)?.into_iter().next().ok_or(RoutingError::NoRoute)
    }
    pub fn routes(&self, destination: NodeId, priority: TrafficPriority, now: u64, max_paths: usize) -> Result<Vec<RoutePath>, RoutingError> {
        if destination == self.local_node { return Ok(vec![RoutePath::new(vec![self.local_node], zero_metrics())]); }
        let limit = max_paths.clamp(1, self.max_paths);
        let mut adjacency: HashMap<NodeId, Vec<TopologyLink>> = HashMap::new();
        for link in self.links.iter().copied().filter(|link| now < link.expires_at) { adjacency.entry(link.from).or_default().push(link); }
        let mut paths: Vec<(Vec<NodeId>, RouteMetrics)> = vec![(vec![self.local_node], zero_metrics())];
        let mut heap = BinaryHeap::new();
        heap.push(SearchState { score: 1.0, node: self.local_node, path_index: 0 });
        let mut results = Vec::new();
        let mut states = 0usize;
        while let Some(state) = heap.pop() {
            states += 1;
            if states > self.max_search_states { return Err(RoutingError::SearchBudgetExceeded); }
            let (nodes, metrics) = paths[state.path_index].clone();
            if state.node == destination {
                results.push(RoutePath::new(nodes, metrics));
                if results.len() >= limit { break; }
                continue;
            }
            if let Some(next_links) = adjacency.get(&state.node) {
                for link in next_links {
                    if nodes.contains(&link.to) { continue; }
                    let combined = combine_metrics(metrics, link.metrics);
                    let mut next_nodes = nodes.clone(); next_nodes.push(link.to);
                    let index = paths.len();
                    paths.push((next_nodes, combined));
                    heap.push(SearchState { score: combined.score(priority), node: link.to, path_index: index });
                }
            }
        }
        results.sort_by(|a, b| b.metrics.score(priority).total_cmp(&a.metrics.score(priority)).then_with(|| a.nodes.len().cmp(&b.nodes.len())));
        Ok(results.into_iter().take(limit).collect())
    }
}

fn metrics_valid(metrics: &RouteMetrics) -> bool { metrics.packet_loss_ppm <= 1_000_000 && metrics.reliability_ppm <= 1_000_000 && metrics.bandwidth_bps > 0 && metrics.hop_count > 0 }
fn zero_metrics() -> RouteMetrics { RouteMetrics { latency_us: 0, jitter_us: 0, packet_loss_ppm: 0, bandwidth_bps: u64::MAX, reliability_ppm: 1_000_000, monetary_cost_micros: 0, energy_cost: 0, hop_count: 0 } }
fn combine_metrics(a: RouteMetrics, b: RouteMetrics) -> RouteMetrics {
    let success_a = 1_000_000u64.saturating_sub(a.packet_loss_ppm as u64);
    let success_b = 1_000_000u64.saturating_sub(b.packet_loss_ppm as u64);
    let loss = 1_000_000u64.saturating_sub(success_a.saturating_mul(success_b) / 1_000_000);
    RouteMetrics { latency_us: a.latency_us.saturating_add(b.latency_us), jitter_us: a.jitter_us.saturating_add(b.jitter_us), packet_loss_ppm: loss.min(1_000_000) as u32, bandwidth_bps: a.bandwidth_bps.min(b.bandwidth_bps), reliability_ppm: a.reliability_ppm.saturating_mul(b.reliability_ppm) / 1_000_000, monetary_cost_micros: a.monetary_cost_micros.saturating_add(b.monetary_cost_micros), energy_cost: a.energy_cost.saturating_add(b.energy_cost), hop_count: a.hop_count.saturating_add(b.hop_count).max(1) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn n(x: u8) -> NodeId { NodeId::from_bytes([x; 32]) }
    fn m(latency: u64) -> RouteMetrics { RouteMetrics { latency_us: latency, jitter_us: 1, packet_loss_ppm: 1000, bandwidth_bps: 1_000_000, reliability_ppm: 999_000, monetary_cost_micros: 1, energy_cost: 1, hop_count: 1 } }
    #[test]
    fn higher_quality_route_wins() {
        let mut r = DistributedRoutingEngine::with_limits(n(1), 16, 32, 8, 4, 128);
        for (origin, target, latency) in [(1,2,10),(2,4,10),(1,3,100),(3,4,100)] {
            r.install_advertisement(
                RouteAdvertisement::with_expiry(n(origin), 1, 0, 100, vec![RouteNeighbor { node_id: n(target), metrics: m(latency) }]),
                10,
            ).unwrap();
        }
        let path = r.best_route(n(4), TrafficPriority::Voice, 10).unwrap();
        assert_eq!(path.nodes, vec![n(1), n(2), n(4)]);
    }
    #[test]
    fn multipath_and_loop_avoidance_are_bounded() {
        let mut r = DistributedRoutingEngine::with_limits(n(1), 16, 32, 8, 4, 128);
        for (origin, target) in [(1,2),(1,3),(2,4),(3,4),(4,1)] {
            r.install_advertisement(
                RouteAdvertisement::with_expiry(n(origin), 1, 0, 100, vec![RouteNeighbor { node_id: n(target), metrics: m(10) }]),
                10,
            ).unwrap();
        }
        let paths = r.routes(n(4), TrafficPriority::Interactive, 10, 4).unwrap();
        assert_eq!(paths.len(), 2);
        assert!(paths.iter().all(|path| path.nodes.iter().collect::<HashSet<_>>().len() == path.nodes.len()));
    }
    #[test]
    fn old_sequence_is_rejected() {
        let mut r = DistributedRoutingEngine::new(n(1));
        assert!(r.install_advertisement(RouteAdvertisement::with_expiry(n(2), 2, 0, 100, vec![]), 10).unwrap());
        assert!(!r.install_advertisement(RouteAdvertisement::with_expiry(n(2), 1, 0, 100, vec![]), 10).unwrap());
    }
}
