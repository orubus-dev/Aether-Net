use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct SessionPolicy {
    pub idle_timeout: Duration,
    pub max_age: Duration,
    pub rekey_after_packets: u64,
}

impl Default for SessionPolicy {
    fn default() -> Self {
        Self {
            idle_timeout: Duration::from_secs(300),

            max_age: Duration::from_secs(3600),

            rekey_after_packets: 1_000_000,
        }
    }
}
