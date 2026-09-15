use crate::network::NodeId;
use std::fmt;
pub const MAX_GATEWAYS: usize = 4_096;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub enum GatewayProtocol{Http,Https,Tcp,Udp,Dns,Custom(u16)}
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum GatewayDirection{Egress,Ingress,Bidirectional}
#[derive(Debug, Clone, PartialEq, Eq)] pub struct GatewayDescriptor{pub node:NodeId,pub protocol:GatewayProtocol,pub direction:GatewayDirection,pub endpoint:[u8;32],pub cost_micros:u64,pub expires_at:u64}
impl GatewayDescriptor{pub fn is_valid_at(&self,now:u64)->bool{now<self.expires_at}}
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum GatewayError{CapacityExceeded,InvalidGateway,NoGateway}
pub struct GatewayDirectory{max_gateways:usize,gateways:Vec<GatewayDescriptor>}
impl GatewayDirectory{
 pub fn new()->Self{Self{max_gateways:MAX_GATEWAYS,gateways:Vec::new()}}
 pub fn with_limit(max:usize)->Self{Self{max_gateways:max.max(1),gateways:Vec::new()}}
 pub fn publish(&mut self,gateway:GatewayDescriptor,now:u64)->Result<bool,GatewayError>{if !gateway.is_valid_at(now){return Err(GatewayError::InvalidGateway)}if let Some(existing)=self.gateways.iter_mut().find(|g|g.node==gateway.node&&g.protocol==gateway.protocol){*existing=gateway;return Ok(true)}if self.gateways.len()>=self.max_gateways{return Err(GatewayError::CapacityExceeded)}self.gateways.push(gateway);Ok(true)}
 pub fn select(&self,protocol:GatewayProtocol,direction:GatewayDirection,now:u64)->Result<&GatewayDescriptor,GatewayError>{self.gateways.iter().filter(|g|g.protocol==protocol&&g.is_valid_at(now)&&(g.direction==direction||g.direction==GatewayDirection::Bidirectional)).min_by(|a,b|a.cost_micros.cmp(&b.cost_micros).then_with(||a.node.as_bytes().cmp(b.node.as_bytes()))).ok_or(GatewayError::NoGateway)}
 pub fn expire(&mut self,now:u64)->usize{let before=self.gateways.len();self.gateways.retain(|g|g.is_valid_at(now));before-self.gateways.len()}
}
impl Default for GatewayDirectory{fn default()->Self{Self::new()}}
impl fmt::Display for GatewayProtocol{fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{match self{Self::Http=>f.write_str("http"),Self::Https=>f.write_str("https"),Self::Tcp=>f.write_str("tcp"),Self::Udp=>f.write_str("udp"),Self::Dns=>f.write_str("dns"),Self::Custom(v)=>write!(f,"custom:{v}")}}}
#[cfg(test)]mod tests{use super::*;fn n(x:u8)->NodeId{NodeId::from_bytes([x;32])}fn g(node:u8,cost:u64)->GatewayDescriptor{GatewayDescriptor{node:n(node),protocol:GatewayProtocol::Https,direction:GatewayDirection::Egress,endpoint:[node;32],cost_micros:cost,expires_at:100}}#[test]fn selects_lowest_cost(){let mut d=GatewayDirectory::new();d.publish(g(1,20),1).unwrap();d.publish(g(2,5),1).unwrap();assert_eq!(d.select(GatewayProtocol::Https,GatewayDirection::Egress,2).unwrap().node,n(2));}#[test]fn expiry_removes_gateway(){let mut d=GatewayDirectory::new();d.publish(g(1,1),1).unwrap();assert_eq!(d.expire(100),1);}}
