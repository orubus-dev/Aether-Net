use crate::network::node::NodeId;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub struct MeshTopology {
    neighbors: HashMap<NodeId, HashSet<NodeId>>,
}

impl MeshTopology {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn connect(&mut self, a: NodeId, b: NodeId) {
        self.neighbors.entry(a).or_default().insert(b);

        self.neighbors.entry(b).or_default().insert(a);
    }

    pub fn disconnect(&mut self, a: &NodeId, b: &NodeId) {
        if let Some(peers) = self.neighbors.get_mut(a) {
            peers.remove(b);
        }

        if let Some(peers) = self.neighbors.get_mut(b) {
            peers.remove(a);
        }
    }

    pub fn neighbors(&self, node: &NodeId) -> impl Iterator<Item = &NodeId> {
        self.neighbors
            .get(node)
            .into_iter()
            .flat_map(|items| items.iter())
    }

    pub fn degree(&self, node: &NodeId) -> usize {
        self.neighbors(node).count()
    }
}
