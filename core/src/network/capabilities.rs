use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeCapabilities(u64);

impl NodeCapabilities {
    pub const NONE: Self = Self(0);

    pub const SEND: Self = Self(1 << 0);
    pub const RECEIVE: Self = Self(1 << 1);
    pub const RELAY: Self = Self(1 << 2);
    pub const ROUTE: Self = Self(1 << 3);
    pub const GATEWAY: Self = Self(1 << 4);
    pub const STORAGE: Self = Self(1 << 5);
    pub const COMPUTE: Self = Self(1 << 6);
    pub const CACHE: Self = Self(1 << 7);
    pub const EMERGENCY: Self = Self(1 << 8);
    pub const SERVICE_HOST: Self = Self(1 << 9);
    pub const OPERATIVE: Self = Self(1 << 10);

    pub const fn bits(self) -> u64 {
        self.0
    }

    pub const fn contains(self, capability: Self) -> bool {
        (self.0 & capability.0) == capability.0
    }

    pub const fn insert(self, capability: Self) -> Self {
        Self(self.0 | capability.0)
    }

    pub const fn remove(self, capability: Self) -> Self {
        Self(self.0 & !capability.0)
    }

    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }
}
