use crate::network::NodeId;
use crate::runtime::discovery::DiscoveryHello;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct PeerEndpoint {
    pub node_id: NodeId,
    pub device_id: [u8; 32],
    pub address: SocketAddr,
    pub last_seen: Instant,
    pub protocol_major: u8,
    pub protocol_minor: u8,
    pub relay_capable: bool,
    pub gateway_capable: bool,
}

#[derive(Debug, Default)]
pub struct PeerTable {
    peers: HashMap<NodeId, PeerEndpoint>,
}

impl PeerTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert(&mut self, hello: &DiscoveryHello, address: SocketAddr, now: Instant) -> NodeId {
        let node_id = NodeId::from_bytes(hello.node_id);

        self.peers.insert(
            node_id,
            PeerEndpoint {
                node_id,
                device_id: hello.device_id,
                address,
                last_seen: now,
                protocol_major: hello.protocol.major,
                protocol_minor: hello.protocol.minor,
                relay_capable: hello
                    .capabilities
                    .contains(crate::network::NodeCapabilities::RELAY),
                gateway_capable: hello
                    .capabilities
                    .contains(crate::network::NodeCapabilities::GATEWAY),
            },
        );

        node_id
    }

    pub fn get(&self, node_id: &NodeId) -> Option<&PeerEndpoint> {
        self.peers.get(node_id)
    }

    pub fn remove(&mut self, node_id: &NodeId) -> Option<PeerEndpoint> {
        self.peers.remove(node_id)
    }

    pub fn expire(&mut self, now: Instant, timeout: std::time::Duration) -> Vec<NodeId> {
        let expired: Vec<NodeId> = self
            .peers
            .iter()
            .filter_map(|(id, peer)| {
                if now.duration_since(peer.last_seen) >= timeout {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        for id in &expired {
            self.peers.remove(id);
        }

        expired
    }

    pub fn len(&self) -> usize {
        self.peers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&NodeId, &PeerEndpoint)> {
        self.peers.iter()
    }
}
