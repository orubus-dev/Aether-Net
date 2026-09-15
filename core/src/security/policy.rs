#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityProfile {
    Standard,
    Private,
    Maximum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecurityPolicy {
    pub profile: SecurityProfile,
    pub require_authenticated_peers: bool,
    pub require_encryption: bool,
    pub allow_plaintext_data: bool,
    pub rotate_privacy_epoch_seconds: u64,
    pub replay_window: u64,
    pub max_packet_size: usize,
}

impl SecurityPolicy {
    pub const fn standard() -> Self {
        Self {
            profile: SecurityProfile::Standard,
            require_authenticated_peers: true,
            require_encryption: true,
            allow_plaintext_data: false,
            rotate_privacy_epoch_seconds: 300,
            replay_window: 128,
            max_packet_size: 65_535,
        }
    }

    pub const fn private() -> Self {
        Self {
            profile: SecurityProfile::Private,
            require_authenticated_peers: true,
            require_encryption: true,
            allow_plaintext_data: false,
            rotate_privacy_epoch_seconds: 120,
            replay_window: 128,
            max_packet_size: 65_535,
        }
    }

    pub const fn maximum() -> Self {
        Self {
            profile: SecurityProfile::Maximum,
            require_authenticated_peers: true,
            require_encryption: true,
            allow_plaintext_data: false,
            rotate_privacy_epoch_seconds: 60,
            replay_window: 128,
            max_packet_size: 65_535,
        }
    }
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self::standard()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_not_plaintext() {
        let policy = SecurityPolicy::default();
        assert!(policy.require_authenticated_peers);
        assert!(policy.require_encryption);
        assert!(!policy.allow_plaintext_data);
    }

    #[test]
    fn private_profile_rotates_more_frequently_than_standard() {
        assert!(SecurityPolicy::private().rotate_privacy_epoch_seconds
            < SecurityPolicy::standard().rotate_privacy_epoch_seconds);
    }
}
