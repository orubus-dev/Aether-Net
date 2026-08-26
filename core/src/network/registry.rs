use crate::network::node::NodeDescriptor;
use crate::network::node::NodeId;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct NodeRegistry {
    nodes: HashMap<NodeId, NodeDescriptor>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, descriptor: NodeDescriptor) -> bool {
        let id = descriptor.node_id;

        let replaced = self.nodes.insert(id, descriptor);

        replaced.is_some()
    }

    pub fn get(&self, node_id: &NodeId) -> Option<&NodeDescriptor> {
        self.nodes.get(node_id)
    }

    pub fn remove(&mut self, node_id: &NodeId) -> Option<NodeDescriptor> {
        self.nodes.remove(node_id)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&NodeId, &NodeDescriptor)> {
        self.nodes.iter()
    }
}
