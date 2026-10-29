// AetherNet number derivation + registry record validation.
//
// Byte-for-byte the same derivation as:
//   number/src/lib.rs          (Rust core)
//   app-android Identity.kt    (Kotlin app)
//   tools/verify_registry.py   (CI verifier)
//
// h = BLAKE2b-256(pubkey || LE64(counter)), free digits = h[0..7] mod 10,
// number = tier prefix + free digits.
//
// BLAKE2b-256 is NOT the first 32 bytes of BLAKE2b-512 — it is a distinct
// parameterization, so node's crypto('blake2b512') cannot be used. The
// vendored @noble/hashes blake2b with dkLen 32 is identical to
// hashlib.blake2b(digest_size=32) and the blake2 crate's Blake2b<U32>.

import { blake2b } from "../_vendor/@noble/hashes/esm/blake2b.js";

export const TIERS = {
  "500": "standard",
  "700": "elite",
  "900": "prestige",
};

export const NUMBER_RE = /^[0-9]{10}$/;
export const HANDLE_RE = /^[a-z0-9]([a-z0-9-]{1,30})[a-z0-9]$/;

export function deriveFreeDigits(pubkeyBytes, counter) {
  const input = new Uint8Array(40);
  input.set(pubkeyBytes, 0);
  new DataView(input.buffer).setBigUint64(32, BigInt(counter), true);
  const h = blake2b(input, { dkLen: 32 });
  let s = "";
  for (let i = 0; i < 7; i++) s += String(h[i] % 10);
  return s;
}

export function deriveNumber(pubkeyBytes, counter) {
  // The tier prefix is part of the record, not the hash; the verifier
  // compares only the seven free digits behind whatever tier prefix the
  // claimant chose. (See tools/verify_registry.py derive_free_digits.)
  return deriveFreeDigits(pubkeyBytes, counter);
}

/** Decode standard base64 to exactly 32 bytes, or return null. */
export function pubkeyFromB64(b64) {
  try {
    const b = Buffer.from(b64, "base64");
    return b.length === 32 ? b : null;
  } catch {
    return null;
  }
}

/**
 * Validate a registry record the same way tools/verify_registry.py does.
 * Returns {ok:true, freeDigits} or {ok:false, error}.
 * Optional fields accepted (schema stays 1): handle, domain, display_name,
 * email_pointer — the CI verifier tolerates unknown keys.
 */
export function validateRecord(rec) {
  if (!rec || typeof rec !== "object") return bad("record must be an object");
  if (rec.schema !== 1) return bad("schema must be 1");
  if (typeof rec.number !== "string" || !NUMBER_RE.test(rec.number))
    return bad("number must be ten digits");
  const prefix = rec.number.slice(0, 3);
  if (!(prefix in TIERS)) return bad("number prefix is not a tier");
  if (rec.tier !== TIERS[prefix]) return bad(`tier must be "${TIERS[prefix]}"`);
  if (typeof rec.pubkey !== "string") return bad("pubkey must be base64");
  const pk = pubkeyFromB64(rec.pubkey);
  if (!pk) return bad("pubkey must decode to 32 bytes");
  if (!Number.isInteger(rec.counter) || rec.counter < 0 || rec.counter > 1_000_000)
    return bad("counter out of range");
  const free = deriveFreeDigits(pk, rec.counter);
  if (free !== rec.number.slice(3))
    return bad(`number does not derive from pubkey+counter (expected ${prefix}${free})`);
  if (rec.handle !== undefined && rec.handle !== null) {
    if (typeof rec.handle !== "string" || !HANDLE_RE.test(rec.handle))
      return bad("handle is not a valid name");
    if (rec.domain !== undefined && rec.domain !== null) {
      const want = `${rec.handle}${prefix === "500" ? ".me.ae" : ".ae"}`;
      if (rec.domain !== want) return bad(`domain must be "${want}"`);
    }
  }
  if (rec.display_name !== undefined && rec.display_name !== null) {
    if (typeof rec.display_name !== "string" || rec.display_name.length > 40)
      return bad("display_name must be a string of at most 40 chars");
  }
  if (rec.email_pointer !== undefined && rec.email_pointer !== null) {
    if (
      typeof rec.email_pointer !== "string" ||
      rec.email_pointer.length > 254 ||
      /\s/.test(rec.email_pointer)
    )
      return bad("email_pointer must be a string of at most 254 chars with no spaces");
  }
  if (typeof rec.created_at !== "string" || Number.isNaN(Date.parse(rec.created_at)))
    return bad("created_at must be an RFC 3339 timestamp");
  return { ok: true, freeDigits: free, prefix };
}

function bad(error) {
  return { ok: false, error };
}
