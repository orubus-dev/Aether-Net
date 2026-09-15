use crate::network::NodeId;
use crate::network::TrafficPriority;
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
pub struct RouteNeighbor { pub node_id: NodeId, pub metrics: RouteMetrics }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TopologyLink { from: NodeId, to: NodeId, metrics: RouteMetrics, expires_at: u64 }

#[derive(Debug, Clone, Copy)]
struct SearchState { score: f64, node: NodeId, path_index: usize }
impl PartialEq for SearchState { fn eq(&self, o: &Self) -> bool { self.score.total_cmp(&o.score)==Ordering::Equal && self.node==o.node && self.path_index==o.path_index } }
impl Eq for SearchState {}
impl Ord for SearchState {
    fn cmp(&self, o: &Self) -> Ordering {
        o.score.total_cmp(&self.score).then_with(|| self.node.as_bytes().cmp(o.node.as_bytes()).reverse()).then_with(|| self.path_index.cmp(&o.path_index).reverse())
    }
}
impl PartialOrd for SearchState { fn partial_cmp(&self, o:&Self)->Option<Ordering>{Some(self.cmp(o))} }

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
        Self { local_node, max_nodes:max_nodes.max(1), max_links:max_links.max(1), max_advertisement_neighbors:max_advertisement_neighbors.max(1), max_paths:max_paths.max(1), max_search_states:max_search_states.max(1), links:Vec::new(), known_sequences:HashMap::new() }
    }
    pub fn local_node(&self)->NodeId{self.local_node}
    pub fn link_count(&self)->usize{self.links.len()}
    pub fn node_count(&self)->usize{let mut s=HashSet::new();s.insert(self.local_node);for l in &self.links{s.insert(l.from);s.insert(l.to);}s.len()}

    pub fn install_advertisement(&mut self, ad: RouteAdvertisement, now:u64)->Result<bool,RoutingError>{
        if !ad.is_valid_at(now) || ad.expires_at<=ad.issued_at || ad.neighbors.len()>self.max_advertisement_neighbors{return Err(RoutingError::InvalidAdvertisement);}
        if ad.neighbors.iter().any(|n| n.node_id==ad.origin || !metrics_valid(&n.metrics)){return Err(RoutingError::InvalidMetrics);}
        if let Some(prev)=self.known_sequences.get(&ad.origin){if ad.sequence<=*prev{return Ok(false);}}
        else if self.node_count()>=self.max_nodes{return Err(RoutingError::NodeTableFull);}
        let origin=ad.origin;
        let neighbors=ad.neighbors;
        let retained=self.links.iter().filter(|l|l.from!=origin).count();
        if retained.saturating_add(neighbors.len())>self.max_links{return Err(RoutingError::LinkTableFull);}
        self.links.retain(|l|l.from!=origin);
        for n in neighbors{self.links.push(TopologyLink{from:origin,to:n.node_id,metrics:n.metrics,expires_at:ad.expires_at});}
        self.known_sequences.insert(origin,ad.sequence);
        Ok(true)
    }

    pub fn remove_origin(&mut self, origin:NodeId)->bool{let b=self.links.len();self.links.retain(|l|l.from!=origin);self.known_sequences.remove(&origin);b!=self.links.len()}
    pub fn expire(&mut self, now:u64)->usize{let b=self.links.len();self.links.retain(|l|now<l.expires_at);b-self.links.len()}
    pub fn advertisement_for(&self, origin:NodeId, sequence:u64, now:u64)->RouteAdvertisement{
        let neighbors=self.links.iter().filter(|l|l.from==origin&&now<l.expires_at).map(|l|RouteNeighbor{node_id:l.to,metrics:l.metrics}).take(self.max_advertisement_neighbors).collect();
        RouteAdvertisement::new(origin,sequence,now,neighbors)
    }
    pub fn best_route(&self,destination:NodeId,priority:TrafficPriority,now:u64)->Result<RoutePath,RoutingError>{self.routes(destination,priority,now,1)?.into_iter().next().ok_or(RoutingError::NoRoute)}
    pub fn routes(&self,destination:NodeId,priority:TrafficPriority,now:u64,max_paths:usize)->Result<Vec<RoutePath>,RoutingError>{
        if destination==self.local_node{return Ok(vec![RoutePath::new(vec![self.local_node],zero_metrics())]);}
        let limit=max_paths.clamp(1,self.max_paths); let mut adj:HashMap<NodeId,Vec<TopologyLink>>=HashMap::new();
        for l in self.links.iter().copied().filter(|l|now<l.expires_at){adj.entry(l.from).or_default().push(l);}
        let mut heap=BinaryHeap::new(); let mut paths:Vec<(Vec<NodeId>,RouteMetrics)>=vec![(vec![self.local_node],zero_metrics())]; let mut results=Vec::new();
        heap.push(SearchState{score:0.0,node:self.local_node,path_index:0}); let mut states=0usize;
        while let Some(st)=heap.pop(){states+=1;if states>self.max_search_states{return Err(RoutingError::SearchBudgetExceeded);} let (nodes,m)=&paths[st.path_index];
            if st.node==destination{results.push(RoutePath::new(nodes.clone(),*m));if results.len()>=limit{break;}continue;}
            if let Some(next)=adj.get(&st.node){for l in next{if nodes.contains(&l.to){continue;}let cm=combine_metrics(*m,l.metrics);let mut nn=nodes.clone();nn.push(l.to);let ix=paths.len();paths.push((nn,cm));heap.push(SearchState{score:cm.score(priority),node:l.to,path_index:ix});}}
        }
        results.sort_by(|a,b|{a.metrics.score(priority).total_cmp(&b.metrics.score(priority)).then_with(||a.nodes.len().cmp(&b.nodes.len()))});Ok(results.into_iter().take(limit).collect())
    }
}

fn metrics_valid(m:&RouteMetrics)->bool{m.packet_loss_ppm<=1_000_000&&m.reliability_ppm<=1_000_000&&m.bandwidth_bps>0&&m.hop_count>0}
fn zero_metrics()->RouteMetrics{RouteMetrics{latency_us:0,jitter_us:0,packet_loss_ppm:0,bandwidth_bps:u64::MAX,reliability_ppm:1_000_000,monetary_cost_micros:0,energy_cost:0,hop_count:0}}
fn combine_metrics(a:RouteMetrics,b:RouteMetrics)->RouteMetrics{RouteMetrics{latency_us:a.latency_us.saturating_add(b.latency_us),jitter_us:a.jitter_us.saturating_add(b.jitter_us),packet_loss_ppm:1_000_000u32.saturating_sub(((1_000_000u64-a.packet_loss_ppm as u64).saturating_mul(1_000_000u64-b.packet_loss_ppm as u64)/1_000_000)as u32),bandwidth_bps:a.bandwidth_bps.min(b.bandwidth_bps),reliability_ppm:((a.reliability_ppm as u64).saturating_mul(b.reliability_ppm as u64)/1_000_000)as u32,monetary_cost_micros:a.monetary_cost_micros.saturating_add(b.monetary_cost_micros),energy_cost:a.energy_cost.saturating_add(b.energy_cost),hop_count:a.hop_count.saturating_add(b.hop_count).max(1)}}

#[cfg(test)]
mod tests{
 use super::*;
 fn n(x:u8)->NodeId{NodeId::from_bytes([x;32])}
 fn m(l:u64)->RouteMetrics{RouteMetrics{latency_us:l,jitter_us:1,packet_loss_ppm:1000,bandwidth_bps:1_000_000,reliability_ppm:999_000,monetary_cost_micros:1,energy_cost:1,hop_count:1}}
 #[test]fn sequence_convergence_rejects_old(){let mut r=DistributedRoutingEngine::with_limits(n(1),16,32,8,4,128);let a=RouteAdvertisement::with_expiry(n(2),2,0,100,vec![]);let b=RouteAdvertisement::with_expiry(n(2),1,0,100,vec![]);assert!(r.install_advertisement(a,10).unwrap());assert!(!r.install_advertisement(b,10).unwrap());}
 #[test]fn shortest_route_uses_lower_latency(){let mut r=DistributedRoutingEngine::with_limits(n(1),16,32,8,4,128);for (o,t,l) in [(1,2,10),(2,4,10),(1,3,100),(3,4,100)]{r.install_advertisement(RouteAdvertisement::with_expiry(n(o),1,0,100,vec![RouteNeighbor{node_id:n(t),metrics:m(l)}]),10).unwrap();}let p=r.best_route(n(4),TrafficPriority::Voice,10).unwrap();assert_eq!(p.nodes,vec![n(1),n(2),n(4)]);assert_eq!(p.metrics.hop_count,2);}
 #[test]fn multipath_returns_alternates(){let mut r=DistributedRoutingEngine::with_limits(n(1),16,32,8,4,128);for (o,t) in [(1,2),(1,3),(2,4),(3,4)]{r.install_advertisement(RouteAdvertisement::with_expiry(n(o),1,0,100,vec![RouteNeighbor{node_id:n(t),metrics:m(10)}]),10).unwrap();}let ps=r.routes(n(4),TrafficPriority::Interactive,10,4).unwrap();assert_eq!(ps.len(),2);assert!(ps.iter().all(|p|{let u=p.nodes.iter().collect::<HashSet<_>>().len();u==p.nodes.len()}));}
 #[test]fn expiration_removes_links(){let mut r=DistributedRoutingEngine::new(n(1));r.install_advertisement(RouteAdvertisement::with_expiry(n(1),1,0,10,vec![RouteNeighbor{node_id:n(2),metrics:m(10)}]),1).unwrap();assert_eq!(r.expire(10),1);}
}
