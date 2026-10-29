// E2E test of the assist envelope scheme — the contract that makes web,
// Android and any libsodium build decrypt each other's mail.
//
// Envelope v1: [1][ephXpk 32][nonce 24][ct+tag]
//   beforenm = HSalsa20(X25519(ephSk, recipXpk), zero16, sigma)   (libsodium crypto_box_beforenm)
//   ct       = XChaCha20-Poly1305-IETF(beforenm, nonce, aad = ephXpk || recipXpk)
//
// This script seals in JS (the web client's code path) and unseals in
// Python via PyNaCl (raw libsodium — the same primitive the Android app
// calls through lazysodium). If that passes, the scheme is libsodium-exact.

import { ed25519, x25519 } from "../web/vendor/node_modules/@noble/curves/esm/ed25519.js";
import { sha512 } from "../web/vendor/node_modules/@noble/hashes/esm/sha512.js";
import { blake2b } from "../web/vendor/node_modules/@noble/hashes/esm/blake2b.js";
import { xchacha20poly1305 } from "../web/vendor/node_modules/@noble/ciphers/esm/chacha.js";
import { readFileSync, writeFileSync } from "node:fs";

const b64 = (u8) => Buffer.from(u8).toString("base64");
const unb64 = (s) => new Uint8Array(Buffer.from(s, "base64"));
const concat = (...a) => {
  const out = new Uint8Array(a.reduce((n, x) => n + x.length, 0));
  let o = 0; for (const x of a) { out.set(x, o); o += x.length; }
  return out;
};
const clampScalar = (d) => { const s = d.slice(0, 32); s[0] &= 248; s[31] &= 127; s[31] |= 64; return s; };
const freeDigits = (pk, counter) => {
  const input = new Uint8Array(40); input.set(pk);
  new DataView(input.buffer).setBigUint64(32, BigInt(counter), true);
  return [...blake2b(input, { dkLen: 32 }).slice(0, 7)].map((x) => x % 10).join("");
};

// ---- HSalsa20 (web/aether.js code path) ----
const SIGMA = new TextEncoder().encode("expand 32-byte k");
const rotl = (x, n) => ((x << n) | (x >>> (32 - n))) >>> 0;
function hsalsa20(key32, in16) {
  const le = (b, o) => (b[o] | (b[o + 1] << 8) | (b[o + 2] << 16) | (b[o + 3] << 24)) >>> 0;
  // classic Salsa20 state: sigma at 0/5/10/15, key at 1-4 & 11-14, input at 6-9
  const x = new Uint32Array(16);
  x[0] = le(SIGMA, 0);
  x[1] = le(key32, 0); x[2] = le(key32, 4); x[3] = le(key32, 8); x[4] = le(key32, 12);
  x[5] = le(SIGMA, 4);
  x[6] = le(in16, 0); x[7] = le(in16, 4); x[8] = le(in16, 8); x[9] = le(in16, 12);
  x[10] = le(SIGMA, 8);
  x[11] = le(key32, 16); x[12] = le(key32, 20); x[13] = le(key32, 24); x[14] = le(key32, 28);
  x[15] = le(SIGMA, 12);
  const qr = (a, b, c, d) => {
    x[b] ^= rotl((x[a] + x[d]) >>> 0, 7);
    x[c] ^= rotl((x[b] + x[a]) >>> 0, 9);
    x[d] ^= rotl((x[c] + x[b]) >>> 0, 13);
    x[a] ^= rotl((x[d] + x[c]) >>> 0, 18);
  };
  for (let i = 0; i < 10; i++) {
    qr(0, 4, 8, 12); qr(5, 9, 13, 1); qr(10, 14, 2, 6); qr(15, 3, 7, 11);
    qr(0, 1, 2, 3); qr(5, 6, 7, 4); qr(10, 11, 8, 9); qr(15, 12, 13, 14);
  }
  const taps = [0, 5, 10, 15, 6, 7, 8, 9];
  const out = new Uint8Array(32);
  taps.forEach((t, i) => {
    out[i * 4] = x[t] & 255; out[i * 4 + 1] = (x[t] >>> 8) & 255;
    out[i * 4 + 2] = (x[t] >>> 16) & 255; out[i * 4 + 3] = (x[t] >>> 24) & 255;
  });
  return out;
}

const ZERO16 = new Uint8Array(16);
const beforenm = (sk, pk) => hsalsa20(x25519.getSharedSecret(sk, pk), ZERO16);

function seal(recipXpk, plaintext, nonce = null) {
  const ephSk = x25519.utils.randomPrivateKey();
  const ephPk = x25519.getPublicKey(ephSk);
  const key = beforenm(ephSk, recipXpk);
  const n = nonce || new Uint8Array(24).map((_, i) => (i * 7 + 3) & 255);
  const ct = xchacha20poly1305(key, n, concat(ephPk, recipXpk)).encrypt(plaintext);
  return concat(new Uint8Array([1]), ephPk, n, ct);
}

// ---- identities (web derivation path) ----
const aliceSeed = new Uint8Array(32).fill(1);
const bobSeed = new Uint8Array(32).fill(2);
const alicePk = ed25519.getPublicKey(aliceSeed);
const bobPk = ed25519.getPublicKey(bobSeed);
const bobXpk = x25519.getPublicKey(clampScalar(sha512(bobSeed)));
const bobXsk = clampScalar(sha512(bobSeed));
const aliceXpk = x25519.getPublicKey(clampScalar(sha512(aliceSeed)));

console.log("alice number:", "500" + freeDigits(alicePk, 0));
console.log("bob   number:", "500" + freeDigits(bobPk, 0));

// ---- seal in JS, unseal in LIBSODIUM (python) ----
const msg = new TextEncoder().encode(JSON.stringify({ kind: "message", text: "hello from the web" }));
const env = seal(bobXpk, msg);
writeFileSync("/tmp/aether_env.json", JSON.stringify({
  envelope: b64(env), recip_xsk: b64(bobXsk), recip_xpk: b64(bobXpk),
}));

const py = await import("node:child_process").then((cp) =>
  cp.execFileSync("python3", [new URL("./unseal_py.py", import.meta.url).pathname]).toString()
);
console.log(py.trim());
if (!py.includes("PY-UNSEAL-OK")) process.exit(1);

// ---- JS unseal roundtrip + tamper/wrong-recipient rejection ----
const ephPk = env.slice(1, 33), nonce = env.slice(33, 57), ct = env.slice(57);
const key = beforenm(bobXsk, ephPk);
const pt = xchacha20poly1305(key, nonce, concat(ephPk, bobXpk)).decrypt(ct);
console.log("js self-unseal:", JSON.parse(new TextDecoder().decode(pt)).text === "hello from the web" ? "PASS" : "FAIL");

const bad = env.slice(); bad[bad.length - 1] ^= 1;
let tamper = false;
try { xchacha20poly1305(beforenm(bobXsk, ephPk), nonce, concat(ephPk, bobXpk)).decrypt(bad.slice(57)); } catch { tamper = true; }
console.log("tamper rejected:", tamper ? "PASS" : "FAIL");

// alice must not read bob's mail
let foreign = false;
try { xchacha20poly1305(beforenm(clampScalar(sha512(aliceSeed)), ephPk), nonce, concat(ephPk, aliceXpk)).decrypt(ct); } catch { foreign = true; }
console.log("wrong recipient rejected:", foreign ? "PASS" : "FAIL");
