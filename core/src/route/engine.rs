use crate::network::node::NodeId;
use crate::network::protocol::TrafficPriority;
use crate::route::path::RoutePath;

#[derive(Debug, Default)]
pub struct RouteEngine;

impl RouteEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn rank(&self, mut routes: Vec<RoutePath>, priority: TrafficPriority) -> Vec<RoutePath> {
        routes.sort_by(|a, b| {
            b.metrics
                .score(priority)
                .total_cmp(&a.metrics.score(priority))
        });

        routes
    }

    pub fn best(&self, routes: Vec<RoutePath>, priority: TrafficPriority) -> Option<RoutePath> {
        self.rank(routes, priority).into_iter().next()
    }

    pub fn remove_loops(&self, path: &RoutePath) -> Option<RoutePath> {
        let mut seen = std::collections::HashSet::new();

        for node in &path.nodes {
            if !seen.insert(*node) {
                return None;
            }
        }

        Some(path.clone())
    }

    pub fn reaches(&self, path: &RoutePath, destination: NodeId) -> bool {
        path.nodes.last().copied() == Some(destination)
    }
}
