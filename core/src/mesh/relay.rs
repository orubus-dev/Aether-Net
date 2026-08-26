use crate::mesh::neighbor::Neighbor;
use crate::network::node::NodeId;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct RelayPolicy {
    pub minimum_battery: u8,
    pub require_thermal_headroom: bool,
    pub prefer_charging_nodes: bool,
}

impl Default for RelayPolicy {
    fn default() -> Self {
        Self {
            minimum_battery: 10,
            require_thermal_headroom: true,
            prefer_charging_nodes: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct RelayTable {
    nodes: HashMap<NodeId, Neighbor>,
}

impl RelayTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, neighbor: Neighbor) {
        self.nodes.insert(neighbor.node_id, neighbor);
    }

    pub fn remove(&mut self, node_id: &NodeId) {
        self.nodes.remove(node_id);
    }

    pub fn eligible(&self, policy: RelayPolicy) -> Vec<&Neighbor> {
        self.nodes
            .values()
            .filter(|node| {
                if policy.require_thermal_headroom && !node.thermal_ok {
                    return false;
                }

                if !node.charging
                    && node
                        .battery_percent
                        .map(|battery| battery <= policy.minimum_battery)
                        .unwrap_or(true)
                {
                    return false;
                }

                node.relay_eligible()
            })
            .collect()
    }

    pub fn best(&self, policy: RelayPolicy) -> Option<&Neighbor> {
        self.eligible(policy).into_iter().max_by(|a, b| {
            a.metrics
                .quality_score()
                .total_cmp(&b.metrics.quality_score())
        })
    }
}
