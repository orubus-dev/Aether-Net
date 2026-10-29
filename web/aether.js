/* AetherNet web client — the privacy carrier in your browser.
 *
 * Keys never leave this device. The assist endpoint is a dumb ciphertext
 * pipe over GitHub storage: it can route your envelopes, but it cannot
 * read them. Number derivation is byte-identical to the Rust core, the
 * Android app and the CI verifier:
 *   digits = BLAKE2b-256(pubkey || LE64(counter))[0..7] mod 10
 */

import { ed25519, x25519 } from "/vendor/node_modules/@noble/curves/esm/ed25519.js";
import { sha512 } from "/vendor/node_modules/@noble/hashes/esm/sha512.js";
import { blake2b } from "/vendor/node_modules/@noble/hashes/esm/blake2b.js";
import { xchacha20poly1305 } from "/vendor/node_modules/@noble/ciphers/esm/chacha.js";

/* ------------------------------ tiny helpers ----------------------------- */

const $ = (id) => document.getElementById(id);
const b64 = (u8) => btoa(String.fromCharCode(...u8));
const unb64 = (s) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));
const concat = (...arrs) => {
  const out = new Uint8Array(arrs.reduce((n, a) => n + a.length, 0));
  let o = 0;
  for (const a of arrs) { out.set(a, o); o += a.length; }
  return out;
};
const rand = (n) => crypto.getRandomValues(new Uint8Array(n));
const DIGITS = (s) => s.replace(/[^0-9]/g, "");

/* -------------------------------- identity ------------------------------- */

const ID_KEY = "aeth.identity";
const EP_KEY = "aeth.endpoint";

let me = null; // { seed, pk, xSk, xPk, number, counter, tier, name, handle, email }
let endpoint = localStorage.getItem(EP_KEY) || location.origin;
if (endpoint.endsWith("/")) endpoint = endpoint.slice(0, -1);

function loadIdentity() {
  const raw = localStorage.getItem(ID_KEY);
  if (!raw) return null;
  try {
    const s = JSON.parse(raw);
    const seed = unb64(s.seed);
    const pk = ed25519.getPublicKey(seed);
    const scalar = clampScalar(sha512(seed));
    return {
      ...s,
      seedB64: s.seed,
      pk,
      pkB64: b64(pk),
      xSk: scalar,
      xPk: x25519.getPublicKey(scalar),
      number: tierPrefix(s.tier) + freeDigits(pk, s.counter),
    };
  } catch {
    return null;
  }
}

function clampScalar(digest) {
  const s = digest.slice(0, 32);
  s[0] &= 248; s[31] &= 127; s[31] |= 64;
  return s;
}

function freeDigits(pk, counter) {
  const input = new Uint8Array(40);
  input.set(pk);
  new DataView(input.buffer).setBigUint64(32, BigInt(counter), true);
  const h = blake2b(input, { dkLen: 32 });
  let s = "";
  for (let i = 0; i < 7; i++) s += String(h[i] % 10);
  return s;
}

const tierPrefix = (t) => ({ standard: "500", elite: "700", prestige: "900" }[t] || "500");
const tierOf = (p) => ({ 500: "standard", 700: "elite", 900: "prestige" }[p] || "standard");
const display = (n) => (n ? `${n.slice(0, 3)} ${n.slice(3, 7)} ${n.slice(7)}` : "—");

function fingerprint(pk) {
  const h = blake2b(pk, { dkLen: 16 });
  return [...h].map((x) => x.toString(16).padStart(2, "0")).join("").replace(/(..)(?=.)/g, "$1 ").trim();
}

/* ----------------------------- seal / unseal ----------------------------- */
/* libsodium-exact envelope v1, shared with the Android app:
     shared = crypto_box_beforenm = HSalsa20(X25519(eph, recip), zero16, sigma)
     ct     = XChaCha20-Poly1305(shared, nonce, aad = ephPk || recipPk)
   Envelope: [v1][ephXpk 32][nonce 24][ct+tag]. */

const SIGMA_BYTES = new TextEncoder().encode("expand 32-byte k");

function rotl(x, n) { return ((x << n) | (x >>> (32 - n))) >>> 0; }

/** HSalsa20 (Bernstein) — the combiner inside libsodium's crypto_box_beforenm. */
function hsalsa20(key32, in16) {
  const le = (b, o) => (b[o] | (b[o + 1] << 8) | (b[o + 2] << 16) | (b[o + 3] << 24)) >>> 0;
  // classic Salsa20 state: sigma at 0/5/10/15, key at 1-4 & 11-14, input at 6-9
  // (validated byte-for-byte against libsodium crypto_box_beforenm)
  const x = new Uint32Array(16);
  x[0] = le(SIGMA_BYTES, 0);
  x[1] = le(key32, 0); x[2] = le(key32, 4); x[3] = le(key32, 8); x[4] = le(key32, 12);
  x[5] = le(SIGMA_BYTES, 4);
  x[6] = le(in16, 0); x[7] = le(in16, 4); x[8] = le(in16, 8); x[9] = le(in16, 12);
  x[10] = le(SIGMA_BYTES, 8);
  x[11] = le(key32, 16); x[12] = le(key32, 20); x[13] = le(key32, 24); x[14] = le(key32, 28);
  x[15] = le(SIGMA_BYTES, 12);
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
  // output tap: 0,5,10,15,6,7,8,9 — 32 bytes
  const taps = [0, 5, 10, 15, 6, 7, 8, 9];
  const out = new Uint8Array(32);
  taps.forEach((t, i) => {
    out[i * 4] = x[t] & 255; out[i * 4 + 1] = (x[t] >>> 8) & 255;
    out[i * 4 + 2] = (x[t] >>> 16) & 255; out[i * 4 + 3] = (x[t] >>> 24) & 255;
  });
  return out;
}

const ZERO16 = new Uint8Array(16);

function beforenm(myXsk, theirXpk) {
  const raw = x25519.getSharedSecret(myXsk, theirXpk);
  return hsalsa20(raw, ZERO16);
}

function seal(recipXpk, plaintext) {
  const ephSk = x25519.utils.randomPrivateKey();
  const ephPk = x25519.getPublicKey(ephSk);
  const key = beforenm(ephSk, recipXpk);
  const nonce = rand(24);
  const ct = xchacha20poly1305(key, nonce, concat(ephPk, recipXpk)).encrypt(plaintext);
  return concat(new Uint8Array([1]), ephPk, nonce, ct);
}

function unseal(blob) {
  if (blob[0] !== 1) throw new Error("envelope version");
  const ephPk = blob.slice(1, 33);
  const nonce = blob.slice(33, 57);
  const ct = blob.slice(57);
  const key = beforenm(me.xSk, ephPk);
  return xchacha20poly1305(key, nonce, concat(ephPk, me.xPk)).decrypt(ct);
}

async function sealToNumber(number, obj) {
  const rec = await resolveNumber(number);
  if (!rec) throw new Error(`${display(number)} is not on the registry yet`);
  const recipX = x25519.getPublicKey(clampScalar(sha512(unb64(rec.pubkey))));
  return b64(seal(recipX, new TextEncoder().encode(JSON.stringify(obj))));
}

/* ----------------------------- assist client ----------------------------- */

async function api(path, opts) {
  const r = await fetch(endpoint + path, opts);
  const j = await r.json().catch(() => ({}));
  j._status = r.status;
  return j;
}

const health = () => api("/api/health");
const resolveNumber = async (n) =>
  (await api(`/api/resolve?name=${n}`)).record || null;
const dropMail = (box, from, blob, kind, ttl) =>
  api("/api/mail", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ box, from, blob, kind, ttl }),
  });
const listMail = (box) => api(`/api/mail?box=${box}`);
const pickMail = (box, id) => api(`/api/mail?box=${box}&id=${id}`);
const dropSignal = (to, from, signal, blob) =>
  api("/api/signal", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ to, from, signal, blob: blob || b64(rand(16)), ttl: 120 }),
  });

/* ------------------------------- onboarding ------------------------------ */

let selTier = "standard";
document.querySelectorAll("#tier-row .chip").forEach((c) =>
  c.addEventListener("click", () => {
    document.querySelectorAll("#tier-row .chip").forEach((x) => x.classList.remove("selected"));
    c.classList.add("selected");
    selTier = c.dataset.tier;
  })
);

$("btn-create").addEventListener("click", () => {
  const name = $("in-name").value.trim();
  const handle = $("in-handle").value.trim().toLowerCase();
  const email = $("in-email").value.trim();
  if (!name) return shake($("in-name"));
  const seed = rand(32);
  const pk = ed25519.getPublicKey(seed);
  me = {
    seed: b64(seed), counter: 0, tier: selTier, name, handle, email,
    pk, pkB64: b64(pk), xSk: clampScalar(sha512(seed)), xPk: x25519.getPublicKey(clampScalar(sha512(seed))),
  };
  me.number = tierPrefix(selTier) + freeDigits(pk, 0);
  $("new-number").textContent = display(me.number);
  $("new-domain").textContent = handle
    ? `your name on the network: ${handle}${selTier === "standard" ? ".me.ae" : ".ae"}`
    : "add a username later to get your .ae domain";
  $("step1").classList.add("hidden");
  $("step2").classList.remove("hidden");
});

$("btn-later").addEventListener("click", () => { saveIdentity(); enterApp(); });

$("btn-claim").addEventListener("click", async () => {
  const st = $("claim-status");
  st.textContent = "claiming…";
  for (let attempt = 0; attempt < 40; attempt++) {
    const rec = {
      number: me.number,
      tier: me.tier,
      pubkey: me.pkB64,
      counter: me.counter,
      created_at: new Date().toISOString(),
      schema: 1,
    };
    if (me.handle) { rec.handle = me.handle; rec.domain = `${me.handle}${me.tier === "standard" ? ".me.ae" : ".ae"}`; }
    if (me.email) rec.email_pointer = me.email;
    const r = await api("/api/claim", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(rec),
    });
    if (r._status === 201) {
      st.textContent = `claimed — you are now globally unique: ${display(me.number)}`;
      saveIdentity();
      setTimeout(enterApp, 900);
      return;
    }
    if (r._status === 409 && /number/.test(r.error || "")) {
      me.counter++;
      me.number = tierPrefix(me.tier) + freeDigits(me.pk, me.counter);
      $("new-number").textContent = display(me.number);
      st.textContent = `collision (astronomically rare) — re-derived as ${display(me.number)}, retrying…`;
      continue;
    }
    st.textContent = r.error || "claim failed — is the endpoint's GITHUB_TOKEN set?";
    return;
  }
});

function saveIdentity() {
  localStorage.setItem(
    ID_KEY,
    JSON.stringify({
      seed: me.seedB64 || me.seed, counter: me.counter, tier: me.tier,
      name: me.name, handle: me.handle, email: me.email || "",
    })
  );
}

/* --------------------------------- shell --------------------------------- */

function enterApp() {
  $("onboarding").classList.add("hidden");
  $("app").classList.remove("hidden");
  $("me-name").textContent = me.name || "anon";
  $("me-number").textContent = `${display(me.number)} · ${me.handle || "no handle"}`;
  fillStatus();
  pollLoop();
}

document.querySelectorAll(".tab").forEach((t) =>
  t.addEventListener("click", () => {
    document.querySelectorAll(".tab").forEach((x) => x.classList.remove("selected"));
    t.classList.add("selected");
    document.querySelectorAll(".view").forEach((v) => v.classList.add("hidden"));
    $(`view-${t.dataset.view}`).classList.remove("hidden");
    if (t.dataset.view === "status") fillStatus();
  })
);

function fillStatus() {
  $("st-number").textContent = display(me.number);
  $("st-handle").textContent = me.handle || "—";
  $("st-domain").textContent = me.handle ? `${me.handle}${me.tier === "standard" ? ".me.ae" : ".ae"}` : "—";
  $("st-tier").textContent = me.tier;
  $("st-email").textContent = me.email || "—";
  $("st-fp").textContent = fingerprint(me.pk);
  $("st-endpoint").textContent = endpoint;
  $("in-endpoint").value = endpoint;
  health().then((h) => {
    const ok = h.ok === true;
    $("st-assist").textContent = ok ? (h.capabilities && h.capabilities.writes ? "healthy · writes on" : "healthy · reads only (no token)") : "unhealthy";
    $("dot-assist").className = "dot " + (ok ? "on" : "warn");
    $("dot-assist-label").textContent = ok ? "assist" : "assist?";
  }).catch(() => {
    $("st-assist").textContent = "unreachable";
    $("dot-assist").className = "dot warn";
  });
  fetch("https://raw.githubusercontent.com/orubus-dev/Aether-Net/main/README.md", { method: "HEAD" })
    .then((r) => { $("st-registry").textContent = r.ok ? "reachable" : "degraded"; $("dot-registry").className = "dot " + (r.ok ? "on" : "warn"); })
    .catch(() => { $("st-registry").textContent = "unreachable"; $("dot-registry").className = "dot warn"; });
  listMail(me.number).then((m) => { $("st-mail").textContent = String(m.live ?? "?"); }).catch(() => {});
}

$("btn-endpoint").addEventListener("click", () => {
  endpoint = $("in-endpoint").value.trim().replace(/\/+$/, "");
  localStorage.setItem(EP_KEY, endpoint);
  fillStatus();
});

$("btn-wipe").addEventListener("click", () => {
  if (confirm("Erase your identity from this device? Your number becomes unreachable. This cannot be undone.")) {
    localStorage.removeItem(ID_KEY);
    localStorage.removeItem("aeth.chats");
    location.reload();
  }
});

function shake(el) {
  el.style.borderColor = "var(--danger)";
  setTimeout(() => (el.style.borderColor = ""), 900);
}

/* --------------------------------- dialer -------------------------------- */

let dialed = "";
document.querySelectorAll(".pad button").forEach((b) =>
  b.addEventListener("click", async () => {
    const d = b.dataset.d;
    if (d === "back") dialed = dialed.slice(0, -1);
    else if (d === "call") return placeCall();
    else if (dialed.length < 10) dialed += d;
    $("dial-num").textContent = display(dialed.padEnd(10, "·"));
  })
);

let activeCall = null; // {peer, state}

async function placeCall() {
  if (dialed.length !== 10) return;
  const st = $("call-status");
  st.textContent = `resolving ${display(dialed)}…`;
  const rec = await resolveNumber(dialed);
  if (!rec) { st.textContent = "not on the network (yet)"; return; }
  st.textContent = "ringing…";
  activeCall = { peer: dialed, state: "ringing", started: Date.now() };
  const r = await dropSignal(dialed, me.number, "ring");
  st.textContent = r._status === 201 ? `ringing ${display(dialed)} — awaiting answer` : `could not ring: ${r.error || "?"}`;
  setTimeout(() => { if (activeCall && activeCall.state === "ringing") { activeCall = null; st.textContent = "no answer"; } }, 30000);
}

$("btn-accept").addEventListener("click", async () => {
  if (!incoming) return;
  await dropSignal(incoming, me.number, "accept");
  openVoice(incoming);
});
$("btn-decline").addEventListener("click", async () => {
  if (incoming) await dropSignal(incoming, me.number, "decline");
  $("incoming").classList.add("hidden");
  incoming = null;
});

function openVoice(peer) {
  $("call-status").textContent = `connected with ${display(peer)} (assist path)`;
  $("voice-panel").classList.remove("hidden");
  activeCall = { peer, state: "live", started: Date.now() };
  renderVoice();
}

/* voice notes: MediaRecorder → sealed envelope → mail */
let recording = null;
$("btn-rec").addEventListener("click", async () => {
  if (recording) { recording.stop(); return; }
  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    const chunks = [];
    recording = new MediaRecorder(stream);
    recording.ondataavailable = (e) => chunks.push(e.data);
    recording.onstop = async () => {
      stream.getTracks().forEach((t) => t.stop());
      $("btn-rec").textContent = "● Record";
      const blob = new Blob(chunks, { type: recording.mimeType });
      if (blob.size > 1_100_000) { $("call-status").textContent = "voice note too long (max ~60s on assist path)"; return; }
      if (activeCall) {
        const buf = new Uint8Array(await blob.arrayBuffer());
        const sealed = await sealToNumber(activeCall.peer, { kind: "voice", mime: blob.type, data: b64(buf) });
        const r = await dropMail(activeCall.peer, me.number, sealed, "voice", 3 * 24 * 3600);
        $("call-status").textContent = r._status === 201 ? "voice note delivered" : "delivery failed";
        addVoiceNote(URL.createObjectURL(blob), "out");
      }
      recording = null;
    };
    recording.start();
    $("btn-rec").textContent = "■ Stop";
  } catch {
    $("call-status").textContent = "microphone unavailable";
    recording = null;
  }
});

function addVoiceNote(url, dir) {
  const div = document.createElement("div");
  div.className = "voicelist";
  const a = document.createElement("audio");
  a.controls = true;
  a.src = url;
  div.appendChild(a);
  $("voice-list").appendChild(div);
}

function renderVoice() {}

/* --------------------------------- chats --------------------------------- */

const CHATS_KEY = "aeth.chats";
const chats = () => JSON.parse(localStorage.getItem(CHATS_KEY) || "{}");
const saveChats = (c) => localStorage.setItem(CHATS_KEY, JSON.stringify(c));

$("btn-start").addEventListener("click", () => {
  const n = DIGITS($("peer-num").value);
  if (n.length !== 10) return shake($("peer-num"));
  const c = chats();
  c[n] = c[n] || [];
  saveChats(c);
  $("peer-num").value = "";
  renderConvs();
  openChat(n);
});

$("btn-back").addEventListener("click", () => {
  $("chat").classList.add("hidden");
  $("conv-list").classList.remove("hidden");
  renderConvs();
});

$("btn-send").addEventListener("click", () => sendMessage());
$("msg").addEventListener("keydown", (e) => { if (e.key === "Enter") sendMessage(); });

let openPeer = null;

async function sendMessage() {
  const text = $("msg").value.trim();
  if (!text || !openPeer) return;
  $("msg").value = "";
  const c = chats();
  c[openPeer].push({ dir: "out", text, ts: Date.now(), state: "sending" });
  saveChats(c);
  renderBubbles();
  try {
    const blob = await sealToNumber(openPeer, { kind: "message", text, ts: Date.now() });
    const r = await dropMail(openPeer, me.number, blob, "message", 3 * 24 * 3600);
    const c2 = chats();
    c2[openPeer][c2[openPeer].length - 1].state = r._status === 201 ? "sent" : "failed";
  } catch (e) {
    const c2 = chats();
    c2[openPeer][c2[openPeer].length - 1].state = "failed: " + e.message;
  }
  saveChats(chats());
  renderBubbles();
}

function openChat(n) {
  openPeer = n;
  $("conv-list").classList.add("hidden");
  $("chat").classList.remove("hidden");
  $("chat-peer").textContent = display(n);
  renderBubbles();
}

function renderConvs() {
  const c = chats();
  const list = $("conv-list");
  list.innerHTML = "";
  const nums = Object.keys(c).sort((a, b) => (c[b].at(-1)?.ts || 0) - (c[a].at(-1)?.ts || 0));
  if (!nums.length) {
    list.innerHTML = `<div class="glass card"><h2>No conversations yet</h2><p class="muted small">Start one by number above — messages are end-to-end encrypted and hop device to device, or ride the assist path when the mesh is out of reach.</p></div>`;
  }
  for (const n of nums) {
    const last = c[n].at(-1) || {};
    const div = document.createElement("div");
    div.className = "conv glass";
    div.innerHTML = `<div><div class="num">${display(n)}</div><div class="preview">${(last.text || "…").replace(/</g, "&lt;")}</div></div><div class="when">${last.ts ? new Date(last.ts).toLocaleDateString() : ""}</div>`;
    div.addEventListener("click", () => openChat(n));
    list.appendChild(div);
  }
}

function renderBubbles() {
  if (!openPeer) return;
  const c = chats();
  const wrap = $("bubbles");
  wrap.innerHTML = "";
  for (const m of c[openPeer] || []) {
    const b = document.createElement("div");
    b.className = `bubble ${m.dir}` + (m.state && m.state.startsWith("fail") ? " pending" : "");
    b.innerHTML = `${(m.text || "").replace(/</g, "&lt;")}<span class="t">${new Date(m.ts).toLocaleTimeString()}${m.state === "sending" ? " · sending" : m.state === "failed" || m.state?.startsWith("failed") ? " · " + m.state : ""}</span>`;
    wrap.appendChild(b);
  }
  wrap.scrollTop = wrap.scrollHeight;
}

/* --------------------------------- sites --------------------------------- */

$("btn-go").addEventListener("click", openSite);
$("site-url").addEventListener("keydown", (e) => { if (e.key === "Enter") openSite(); });

async function openSite() {
  const name = $("site-url").value.trim().toLowerCase();
  if (!name) return;
  $("site-out").classList.add("hidden");
  $("site-frame").classList.remove("hidden");
  const r = await api(`/api/sites?name=${encodeURIComponent(name)}&path=/`);
  if (!r.ok) {
    $("site-frame").classList.add("hidden");
    $("site-out").classList.remove("hidden");
    $("site-pre").textContent = `cannot open ${name}: ${r.error || "?"}\n\nA site lives on the registry as a content-addressed package and is cached by relays. If it was published recently, give the mesh a moment.`;
    return;
  }
  const html = new TextDecoder().decode(unb64(r.content));
  // rewrite relative links so navigation stays on aether
  const base = name.replace(/\.ae$/, "") + "";
  const doc = html.replace(/(href|src)=["'](?!https?:|data:|#|\/)([^"']+)["']/g, `$1="/api/sites?name=${encodeURIComponent(name)}&path=$2"`);
  $("site-frame").srcdoc = doc;
}

$("btn-publish").addEventListener("click", async () => {
  const st = $("pub-status");
  if (!me.handle) { st.textContent = "claim a username first — your domain is your namespace"; return; }
  const files = [...$("site-files").files];
  if (!files.length) { st.textContent = "pick files (index.html required)"; return; }
  st.textContent = "packing…";
  const payload = [];
  for (const f of files.slice(0, 64)) {
    const buf = new Uint8Array(await f.arrayBuffer());
    payload.push({ path: f.name, content: b64(buf) });
  }
  st.textContent = "deploying (one atomic commit)…";
  const r = await api("/api/sites", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ name: me.handle, files: payload }),
  });
  if (r._status === 201) {
    st.textContent = `live at ${r.domain} — open it in the Sites bar`;
    $("site-url").value = r.domain;
    openSite();
  } else {
    st.textContent = r.error || "publish failed";
  }
});

/* -------------------------------- polling -------------------------------- */

let incoming = null;

async function pollOnce() {
  if (!me) return;
  try {
    const m = await listMail(me.number);
    if ($("st-mail")) $("st-mail").textContent = String(m.live ?? "?");
    for (const env of m.envelopes || []) {
      const picked = await pickMail(me.number, env.id);
      if (!picked.envelope) continue;
      let inner;
      try {
        inner = JSON.parse(new TextDecoder().decode(unseal(unb64(picked.envelope.blob))));
      } catch {
        continue; // not for our key / corrupted — drop silently
      }
      handleEnvelope(picked.envelope, inner);
    }
  } catch { /* endpoint asleep; retry next tick */ }
}

function handleEnvelope(env, inner) {
  if (inner.kind === "message") {
    const c = chats();
    c[env.from] = c[env.from] || [];
    c[env.from].push({ dir: "in", text: inner.text, ts: inner.ts || Date.now() });
    saveChats(c);
    if (openPeer === env.from) renderBubbles(); else renderConvs();
  } else if (env.kind === "signal") {
    if (env.signal === "ring" && !activeCall) {
      incoming = env.from;
      $("incoming").classList.remove("hidden");
      $("incoming-num").textContent = display(env.from);
      $("incoming-num").dataset.num = env.from;
    } else if (env.signal === "accept" && activeCall && activeCall.peer === env.from) {
      openVoice(env.from);
    } else if (env.signal === "decline" && activeCall && activeCall.peer === env.from) {
      activeCall = null;
      $("call-status").textContent = "call declined";
    }
  } else if (inner.kind === "voice" && activeCall && activeCall.peer === env.from) {
    const bin = Uint8Array.from(atob(inner.data), (c) => c.charCodeAt(0));
    addVoiceNote(URL.createObjectURL(new Blob([bin], { type: inner.mime })), "in");
  }
}

function pollLoop() {
  pollOnce();
  setInterval(pollOnce, 8000);
  setInterval(() => { // keep call ring fresh
    if (activeCall && activeCall.state === "ringing") dropSignal(activeCall.peer, me.number, "ring").catch(() => {});
  }, 15000);
}

/* --------------------------------- boot ---------------------------------- */

me = loadIdentity();
if (me) enterApp();
else $("onboarding").classList.remove("hidden");
