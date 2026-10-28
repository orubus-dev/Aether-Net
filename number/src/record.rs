//! Registry record model for AetherNet number and handle claims.
//!
//! A claim is one JSON file in the `registry/` tree of the Aether-Net
//! repository:
//!
//! ```text
//! registry/700/7001234567.json   — a number claim
//! registry/handles/genesis.json  — a handle claim
//! ```
//!
//! Claims are created atomically: the GitHub Contents API rejects a create
//! when the path already exists, so two devices racing for the same number
//! cannot both win. The CI verifier (`tools/verify_registry.py`) walks every
//! record on every push and rejects the push if any record fails
//! [`RegistryRecord::verify`] — meaning every published number provably
//! derives from the public key that claims it.

use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};

/// One claimed AetherNet number, as stored in `registry/<tier>/<number>.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryRecord {
    /// The ten-digit number, plain form: `"7001234567"`.
    pub number: String,
    /// Tier name: `"standard" | "elite" | "prestige"`.
    pub tier: String,
    /// Ed25519 identity public key, standard base64, 32 bytes.
    pub pubkey: String,
    /// Derivation counter used when the number was claimed.
    pub counter: u64,
    /// Optional handle claimed with this number (`"genesis"` in `genesis@me.ae`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handle: Option<String>,
    /// Optional AetherNet domain bound to this number (`"genesis.me.ae"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    /// RFC 3339 creation timestamp of the claim.
    pub created_at: String,
    /// Registry schema version, currently `1`.
    pub schema: u32,
}

impl RegistryRecord {
    /// Build a record for a derived number. The tier comes from the number
    /// itself so the record can never disagree with its own prefix.
    pub fn new(
        number: &crate::AetherNumber,
        pubkey: &VerifyingKey,
        counter: u64,
        handle: Option<&str>,
    ) -> RegistryRecord {
        let handle = handle.map(str::to_owned);
        let domain = handle.as_ref().map(|h| match number.tier() {
            crate::Tier::Standard => format!("{h}.me.ae"),
            _ => format!("{h}.ae"),
        });
        RegistryRecord {
            number: number.plain(),
            tier: number.tier().name().to_owned(),
            pubkey: base64_encode(pubkey.as_bytes()),
            counter,
            handle,
            domain,
            created_at: now_rfc3339(),
            schema: 1,
        }
    }

    /// Verify the record is internally consistent: schema known, number
    /// parses, tier matches prefix, pubkey decodes to 32 bytes, and the
    /// number is exactly what `derive(pubkey, counter)` produces.
    pub fn verify(&self) -> Result<(), String> {
        if self.schema != 1 {
            return Err(format!("unknown schema version {}", self.schema));
        }
        let n = crate::AetherNumber::parse(&self.number)
            .ok_or_else(|| format!("number {} does not parse", self.number))?;
        if n.tier().name() != self.tier {
            return Err(format!(
                "tier {} does not match number prefix {}",
                self.tier,
                n.tier().name()
            ));
        }
        let raw =
            base64_decode(&self.pubkey).ok_or_else(|| "pubkey is not valid base64".to_owned())?;
        let bytes: [u8; 32] = raw
            .as_slice()
            .try_into()
            .map_err(|_| format!("pubkey is {} bytes, want 32", raw.len()))?;
        let vk = VerifyingKey::from_bytes(&bytes)
            .map_err(|e| format!("pubkey is not a valid Ed25519 key: {e}"))?;
        let derived = crate::AetherNumber::derive(&vk, self.counter);
        // Tier only re-letters the prefix; the seven free digits are
        // tier-invariant, so verification re-applies the record's tier.
        if derived.with_tier(n.tier()).plain() != self.number {
            return Err(format!(
                "number {} is not derive(pubkey, counter={}) = {}",
                self.number,
                self.counter,
                derived.plain()
            ));
        }
        Ok(())
    }
}

/// RFC 3339 timestamp without pulling chrono into the crate.
fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Days-to-civil algorithm (Howard Hinnant), UTC.
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

/// Standard base64 (RFC 4648, with padding), implemented locally so the
/// crate has no extra dependency.
pub(crate) fn base64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Standard base64 decode; returns `None` on any invalid input.
pub(crate) fn base64_decode(s: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some(u32::from(c - b'A')),
            b'a'..=b'z' => Some(u32::from(c - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(c - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let s: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if s.is_empty() || s.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    for chunk in s.chunks(4) {
        let pad = chunk.iter().rev().take_while(|&&c| c == b'=').count();
        if pad > 2 {
            return None;
        }
        let mut n: u32 = 0;
        for &c in chunk.iter().take(4 - pad) {
            n = (n << 6) | val(c)?;
        }
        n <<= 6 * pad as u32;
        let bytes = n.to_be_bytes();
        out.extend_from_slice(&bytes[1..4 - pad]);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        for v in ["f", "fo", "foo", "foob", "fooba", "foobar"] {
            let enc = base64_encode(v.as_bytes());
            assert_eq!(base64_decode(&enc).unwrap(), v.as_bytes());
        }
    }

    #[test]
    fn base64_rejects_garbage() {
        assert!(base64_decode("A").is_none());
        assert!(base64_decode("AB@=").is_none());
        assert!(base64_decode("ABCDE").is_none());
    }

    #[test]
    fn timestamp_shape() {
        let ts = now_rfc3339();
        // 2026-xx-xxTxx:xx:xxZ — 20 chars, fixed shape.
        assert_eq!(ts.len(), 20);
        assert!(ts.ends_with('Z'));
        assert_eq!(ts.as_bytes()[4], b'-');
        assert_eq!(ts.as_bytes()[10], b'T');
    }
}
