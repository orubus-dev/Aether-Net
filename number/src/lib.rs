//! AetherNet number allocation.
//!
//! AetherNet numbers are ten-digit device identities on the AetherNet carrier
//! network, allocated from tiered ranges:
//!
//! - `500 ...` — standard tier
//! - `700 ...` — elite tier
//! - `900 ...` — prestige tier
//!
//! Numbers are never drawn from a random pool. They are derived from the
//! device's Ed25519 identity public key with a bounded retry counter, so a
//! device computes the same number on every boot and two devices deriving the
//! same number is astronomically unlikely. The GitHub registry is the second
//! guard: a claim is an atomic file create (`registry/<tier>/<number>.json`),
//! and the registry CI verifier recomputes every stored record from its
//! pubkey+counter, so a number that exists must be exactly the one its key
//! derives.

pub mod record;

use blake2::{
    Blake2b,
    digest::{Digest, consts::U32},
};

use ed25519_dalek::VerifyingKey;

/// Tier encoded by the first three digits of a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier {
    /// `500 ...` — standard tier. Default allocation for every device.
    Standard,
    /// `700 ...` — elite tier. Relay priority and hosting privileges.
    Elite,
    /// `900 ...` — prestige tier. Highest relay priority, custom `.ae` names.
    Prestige,
}

impl Tier {
    /// The three-digit prefix that opens the number.
    pub const fn prefix(self) -> u32 {
        match self {
            Tier::Standard => 500,
            Tier::Elite => 700,
            Tier::Prestige => 900,
        }
    }

    /// Domain this tier earns on the AetherNet name space.
    pub const fn domain_suffix(self) -> &'static str {
        match self {
            Tier::Standard => "me.ae",
            Tier::Elite => "ae",
            Tier::Prestige => "ae",
        }
    }

    /// Parse a tier from its three-digit prefix.
    pub fn from_prefix(prefix: u32) -> Option<Tier> {
        match prefix {
            500 => Some(Tier::Standard),
            700 => Some(Tier::Elite),
            900 => Some(Tier::Prestige),
            _ => None,
        }
    }

    /// Parse a tier from a lowercase name (`"standard"`, `"elite"`, `"prestige"`).
    pub fn from_name(name: &str) -> Option<Tier> {
        match name {
            "standard" => Some(Tier::Standard),
            "elite" => Some(Tier::Elite),
            "prestige" => Some(Tier::Prestige),
            _ => None,
        }
    }

    /// Lowercase tier name used in registry records.
    pub const fn name(self) -> &'static str {
        match self {
            Tier::Standard => "standard",
            Tier::Elite => "elite",
            Tier::Prestige => "prestige",
        }
    }
}

/// A derived AetherNet number.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AetherNumber {
    tier: Tier,
    digits: [u8; 7],
}

/// Maximum retry counter before derivation gives up. With a 10^7 space per
/// tier the birthday bound sits near 3.7k live claims; the atomic registry
/// claim resolves the rare collision by bumping the counter and re-deriving,
/// so in practice the first or second counter value wins.
pub const MAX_DERIVATION_RETRIES: u64 = 1_000_000;

impl AetherNumber {
    /// Derive the number for `pubkey` at `counter`.
    ///
    /// `h = BLAKE2b-256(pubkey || LE64(counter))`, then the seven free digits
    /// are `h[0..7] % 10`. Deterministic on every platform; the Kotlin app
    /// computes exactly this.
    pub fn derive(pubkey: &VerifyingKey, counter: u64) -> AetherNumber {
        let mut input = [0u8; 40];
        input[..32].copy_from_slice(pubkey.as_bytes());
        input[32..].copy_from_slice(&counter.to_le_bytes());

        let mut h = Blake2b::<U32>::new();
        h.update(&input);
        let out = h.finalize();

        let mut digits = [0u8; 7];
        for (i, d) in digits.iter_mut().enumerate() {
            *d = out[i] % 10;
        }
        AetherNumber {
            tier: Tier::Standard,
            digits,
        }
    }

    /// Derive with collision retry: returns the number for the first counter
    /// in `0..=MAX_DERIVATION_RETRIES` that `accept` lets through. The app
    /// accepts when the atomic registry claim succeeds.
    pub fn derive_with_retry<F: Fn(&AetherNumber) -> bool>(
        pubkey: &VerifyingKey,
        accept: F,
    ) -> Option<(AetherNumber, u64)> {
        for counter in 0..=MAX_DERIVATION_RETRIES {
            let candidate = AetherNumber::derive(pubkey, counter);
            if accept(&candidate) {
                return Some((candidate, counter));
            }
        }
        None
    }

    /// Attach a tier. Tier changes the prefix only; the seven free digits are
    /// identical across tiers for the same key, which keeps tier upgrades a
    /// registry-side re-claim instead of a new identity.
    pub fn with_tier(mut self, tier: Tier) -> AetherNumber {
        self.tier = tier;
        self
    }

    /// The tier this number belongs to.
    pub const fn tier(&self) -> Tier {
        self.tier
    }

    /// The ten digits as a plain string, e.g. `"7001234567"`.
    pub fn plain(&self) -> String {
        let mut s = String::with_capacity(10);
        s.push_str(&format!("{:03}", self.tier.prefix()));
        for d in self.digits {
            s.push((b'0' + d) as char);
        }
        s
    }

    /// The number in display grouping, e.g. `"700 1234 567"`.
    pub fn display(&self) -> String {
        let p = self.plain();
        format!("{} {} {}", &p[0..3], &p[3..7], &p[7..10])
    }

    /// Parse a ten-digit number (`"7001234567"`, `"700 1234 567"`, or with
    /// dashes). Returns `None` for any wrong length, non-digit or untiered
    /// prefix.
    pub fn parse(s: &str) -> Option<AetherNumber> {
        let plain: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
        if plain.len() != 10 {
            return None;
        }
        let prefix: u32 = plain[0..3].parse().ok()?;
        let tier = Tier::from_prefix(prefix)?;
        let mut digits = [0u8; 7];
        for (i, d) in digits.iter_mut().enumerate() {
            *d = plain.as_bytes()[3 + i].wrapping_sub(b'0');
            if *d > 9 {
                return None;
            }
        }
        Some(AetherNumber { tier, digits })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    #[test]
    fn derivation_is_deterministic() {
        let k = key(1);
        let a = AetherNumber::derive(&k.verifying_key(), 0);
        let b = AetherNumber::derive(&k.verifying_key(), 0);
        assert_eq!(a, b);
        assert_eq!(a.plain().len(), 10);
    }

    #[test]
    fn different_keys_derive_different_numbers() {
        let a = AetherNumber::derive(&key(1).verifying_key(), 0);
        let b = AetherNumber::derive(&key(2).verifying_key(), 0);
        assert_ne!(a, b);
    }

    #[test]
    fn counter_changes_the_number() {
        let k = key(3).verifying_key();
        let a = AetherNumber::derive(&k, 0);
        let b = AetherNumber::derive(&k, 1);
        assert_ne!(a, b);
    }

    #[test]
    fn display_grouping_matches_plan() {
        let n = AetherNumber::derive(&key(4).verifying_key(), 0).with_tier(Tier::Elite);
        let d = n.display();
        assert_eq!(d.chars().count(), 12);
        assert!(d.starts_with("700 "));
        assert_eq!(d.chars().filter(|c| *c == ' ').count(), 2);
    }

    #[test]
    fn parse_roundtrips() {
        for seed in 0u8..16 {
            let n = AetherNumber::derive(&key(seed).verifying_key(), seed as u64);
            let parsed = AetherNumber::parse(&n.plain()).expect("parses");
            assert_eq!(parsed, n);
            let parsed = AetherNumber::parse(&n.display()).expect("parses grouped");
            assert_eq!(parsed, n);
        }
    }

    #[test]
    fn parse_rejects_bad_input() {
        assert!(AetherNumber::parse("").is_none());
        assert!(AetherNumber::parse("123456789").is_none());
        assert!(AetherNumber::parse("12345678901").is_none());
        assert!(AetherNumber::parse("4001234567").is_none());
        assert!(AetherNumber::parse("700123456").is_none());
    }

    #[test]
    fn tiers_map_to_prefixes_and_domains() {
        assert_eq!(Tier::Standard.prefix(), 500);
        assert_eq!(Tier::Elite.prefix(), 700);
        assert_eq!(Tier::Prestige.prefix(), 900);
        assert_eq!(Tier::from_prefix(700), Some(Tier::Elite));
        assert_eq!(Tier::from_prefix(600), None);
        assert_eq!(Tier::Standard.domain_suffix(), "me.ae");
        assert_eq!(Tier::Elite.domain_suffix(), "ae");
        assert_eq!(Tier::from_name("prestige"), Some(Tier::Prestige));
    }

    #[test]
    fn tier_only_changes_prefix() {
        let k = key(9).verifying_key();
        let standard = AetherNumber::derive(&k, 0);
        let elite = standard.clone().with_tier(Tier::Elite);
        assert_eq!(standard.plain()[3..], elite.plain()[3..]);
        assert_ne!(&standard.plain()[..3], &elite.plain()[..3]);
    }

    #[test]
    fn retry_finds_first_accepted_counter() {
        let k = key(11).verifying_key();
        let first = AetherNumber::derive(&k, 0);
        // Nothing accepted => derivation keeps probing and returns None.
        let none = AetherNumber::derive_with_retry(&k, |_| false);
        assert!(none.is_none());

        let (n, counter) =
            AetherNumber::derive_with_retry(&k, |c| c.plain() != first.plain()).unwrap();
        assert!(counter >= 1);
        assert_ne!(n, first);
    }

    #[test]
    fn registry_record_roundtrip() {
        let k = key(12);
        let n = AetherNumber::derive(&k.verifying_key(), 2).with_tier(Tier::Elite);
        let rec = record::RegistryRecord::new(&n, &k.verifying_key(), 2, Some("genesis"));
        let json = serde_json::to_string_pretty(&rec).unwrap();
        let back: record::RegistryRecord = serde_json::from_str(&json).unwrap();
        back.verify().expect("record verifies");
    }

    #[test]
    fn tampered_record_fails_verification() {
        let k = key(13);
        let n = AetherNumber::derive(&k.verifying_key(), 0);
        let mut rec = record::RegistryRecord::new(&n, &k.verifying_key(), 0, None);
        rec.counter = 5; // number no longer matches pubkey+counter
        assert!(rec.verify().is_err());
    }
}
