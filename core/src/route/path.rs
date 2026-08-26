use crate::network::node::NodeId;
use crate::route::metric::RouteMetrics;

#[derive(Debug, Clone)]
pub struct RoutePath {
    pub nodes: Vec<NodeId>,
    pub metrics: RouteMetrics,
}

impl RoutePath {
    pub fn new(nodes: Vec<NodeId>, metrics: RouteMetrics) -> Self {
        Self { nodes, metrics }
    }

    pub fn hop_count(&self) -> usize {
        self.nodes.len().saturating_sub(1)
    }

    pub fn contains(&self, node: &NodeId) -> bool {
        self.nodes.contains(node)
    }
}
