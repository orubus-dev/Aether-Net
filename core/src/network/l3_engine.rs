use super::{AetherAddress, AetherL3Packet, L3Error};
use crate::network::TrafficPriority;

pub const DEFAULT_MAX_ROUTES: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct L3Route {
    pub destination: AetherAddress,
    pub next_hop: AetherAddress,
    pub metric: u32,
    pub learned_at: u64,
    pub expires_at: Option<u64>,
}

impl L3Route {
    pub const fn new(destination: AetherAddress, next_hop: AetherAddress, metric: u32, learned_at: u64, expires_at: Option<u64>) -> Self {
        Self { destination, next_hop, metric, learned_at, expires_at }
    }
    pub fn is_valid_at(&self, now: u64) -> bool { self.expires_at.map_or(true, |expiry| now < expiry) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct L3ForwardDecision {
    pub next_hop: AetherAddress,
    pub priority: TrafficPriority,
    pub remaining_hops: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L3Delivery { Local, Forward(L3ForwardDecision) }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L3EngineError { RouteTableFull, InvalidRoute, HopLimitExpired, NoRoute }

pub struct AetherL3Engine {
    local_address: AetherAddress,
    max_routes: usize,
    routes: Vec<L3Route>,
}

impl AetherL3Engine {
    pub fn new(local_address: AetherAddress) -> Self { Self::with_capacity(local_address, DEFAULT_MAX_ROUTES) }
    pub fn with_capacity(local_address: AetherAddress, max_routes: usize) -> Self { Self { local_address, max_routes: max_routes.max(1), routes: Vec::new() } }
    pub const fn local_address(&self) -> AetherAddress { self.local_address }
    pub fn route_count(&self) -> usize { self.routes.len() }

    pub fn install_route(&mut self, route: L3Route) -> Result<(), L3EngineError> {
        if route.destination.is_zero() || route.next_hop.is_zero() || route.metric == 0 { return Err(L3EngineError::InvalidRoute); }
        if let Some(existing) = self.routes.iter_mut().find(|candidate| candidate.destination == route.destination) {
            if route.metric < existing.metric || (route.metric == existing.metric && route.learned_at >= existing.learned_at) { *existing = route; }
            return Ok(());
        }
        if self.routes.len() >= self.max_routes { return Err(L3EngineError::RouteTableFull); }
        self.routes.push(route);
        Ok(())
    }

    pub fn remove_route(&mut self, destination: AetherAddress) -> bool {
        let before = self.routes.len();
        self.routes.retain(|route| route.destination != destination);
        before != self.routes.len()
    }

    pub fn expire_routes(&mut self, now: u64) -> usize {
        let before = self.routes.len();
        self.routes.retain(|route| route.is_valid_at(now));
        before - self.routes.len()
    }

    pub fn lookup_route(&self, destination: AetherAddress, now: u64) -> Option<L3Route> {
        self.routes.iter().filter(|route| route.destination == destination && route.is_valid_at(now)).min_by_key(|route| (route.metric, std::cmp::Reverse(route.learned_at))).copied()
    }

    pub fn prepare(&self, packet: &mut AetherL3Packet, now: u64) -> Result<L3Delivery, L3EngineError> {
        if packet.destination == self.local_address { return Ok(L3Delivery::Local); }
        let route = self.lookup_route(packet.destination, now).ok_or(L3EngineError::NoRoute)?;
        if packet.hop_limit <= 1 { return Err(L3EngineError::HopLimitExpired); }
        packet.forward().map_err(|error| match error { L3Error::InvalidHopLimit => L3EngineError::HopLimitExpired, _ => L3EngineError::InvalidRoute })?;
        Ok(L3Delivery::Forward(L3ForwardDecision { next_hop: route.next_hop, priority: packet.priority, remaining_hops: packet.hop_limit }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::{AETHER_ADDRESS_LENGTH, L3Flags};
    fn address(seed: u8) -> AetherAddress { AetherAddress::from_bytes([seed; AETHER_ADDRESS_LENGTH]) }
    fn packet(destination: AetherAddress, hop_limit: u8) -> AetherL3Packet { AetherL3Packet::new(address(1), destination, TrafficPriority::Interactive, L3Flags::RELAYABLE, hop_limit, vec![1,2,3]).unwrap() }
    #[test] fn local_destination_is_not_forwarded(){let engine=AetherL3Engine::new(address(2));let mut packet=packet(address(2),1);assert_eq!(engine.prepare(&mut packet,100).unwrap(),L3Delivery::Local);assert_eq!(packet.hop_limit,1)}
    #[test] fn forwarding_selects_route_and_consumes_one_hop(){let mut engine=AetherL3Engine::new(address(1));engine.install_route(L3Route::new(address(3),address(2),10,1,None)).unwrap();let mut packet=packet(address(3),4);let decision=engine.prepare(&mut packet,100).unwrap();assert_eq!(packet.hop_limit,3);assert_eq!(decision,L3Delivery::Forward(L3ForwardDecision{next_hop:address(2),priority:TrafficPriority::Interactive,remaining_hops:3}))}
    #[test] fn direct_route_is_valid(){let mut engine=AetherL3Engine::new(address(1));engine.install_route(L3Route::new(address(2),address(2),1,1,None)).unwrap();assert!(engine.lookup_route(address(2),1).is_some())}
    #[test] fn expired_route_is_not_used(){let mut engine=AetherL3Engine::new(address(1));engine.install_route(L3Route::new(address(3),address(2),10,1,Some(50))).unwrap();let mut packet=packet(address(3),4);assert_eq!(engine.prepare(&mut packet,50).unwrap_err(),L3EngineError::NoRoute)}
    #[test] fn route_table_is_bounded(){let mut engine=AetherL3Engine::with_capacity(address(1),2);engine.install_route(L3Route::new(address(2),address(3),1,1,None)).unwrap();engine.install_route(L3Route::new(address(3),address(4),1,1,None)).unwrap();assert_eq!(engine.install_route(L3Route::new(address(4),address(5),1,1,None)).unwrap_err(),L3EngineError::RouteTableFull)}
}
