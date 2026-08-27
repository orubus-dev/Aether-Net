use crate::network::NodeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameHostingMode {
    Free,
    Paid,
}

#[derive(Debug, Clone)]
pub struct GameHostOffer {
    pub host: NodeId,

    pub game_id: [u8; 32],

    pub mode: GameHostingMode,

    pub price_micros_per_hour: u64,

    pub max_players: u16,

    pub max_bandwidth_bps: u64,

    pub authoritative: bool,
}

impl GameHostOffer {
    pub fn available(&self) -> bool {
        self.max_players > 0 && self.max_bandwidth_bps > 0
    }
}
