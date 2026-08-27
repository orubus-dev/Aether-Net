use crate::network::NodeId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForwardCandidate {
    pub node: NodeId,

    pub latency_ms: f64,

    pub jitter_ms: f64,

    pub loss: f64,

    pub bandwidth_bps: u64,

    pub reliability: f64,
}

impl ForwardCandidate {
    pub fn score(&self) -> f64 {
        let latency = self.latency_ms.max(0.1);

        let jitter = self.jitter_ms.max(0.1);

        let bandwidth = (self.bandwidth_bps as f64).max(1.0).log10();

        let reliability = self.reliability.clamp(0.0, 1.0);

        (bandwidth * 5.0) + (reliability * 20.0)
            - (latency * 0.5)
            - (jitter * 0.25)
            - (self.loss * 100.0)
    }
}

pub struct ForwardSelector;

impl ForwardSelector {
    pub fn best(candidates: &[ForwardCandidate]) -> Option<ForwardCandidate> {
        candidates.iter().copied().max_by(|left, right| {
            left.score()
                .partial_cmp(&right.score())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}
