/* AetherNet web client — the privacy carrier in your browser.
 *
 * Keys never leave this device. The assist endpoint is a dumb ciphertext
 * pipe over GitHub storage: it can route your envelopes, but it cannot
 * read them. Number derivation is byte-identical to the Rust core, the
 * Android app and the CI verifier:
 *   digits = BLAKE2b-256(pubkey || LE64(counter))[0..7] mod 10
 *
 * Calls: signaling rides sealed envelopes on the assist path; live audio
 * is peer-to-peer WebRTC (DTLS-SRTP — end-to-end encrypted by protocol).
 * The Rust core maps this 1:1 onto native L4 media sessions.
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
const mmss = (sec) => `${String(Math.floor(sec / 60)).padStart(2, "0")}:${String(sec % 60).padStart(2, "0")}`;

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

function rotl(x, n) { return ((x << n) | (x >>> 32 - n)) >>> 0; }

/** HSalsa20 (Bernstein) — the combiner inside libsodium's crypto_box_beforenm. */
function hsalsa20(key32, in16) {
  const le = (b, o) => (b[o] | (b[o + 1] << 8) | (b[o + 2] << 16) | (b[o + 3] << 24)) >>> 0;
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
const listSignals = (box) => api(`/api/signal?box=${box}`);
const pickSignal = (box, id) => api(`/api/signal?box=${box}&id=${id}`);
const heartbeat = (number, fp, name) =>
  api("/api/presence", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ number, fingerprint: fp, display_name: name }),
  });
const directory = () => api("/api/presence");

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
  heartbeat(me.number, fingerprint(me.pk), me.name || "peer").catch(() => {});
  pollLoop();
}

document.querySelectorAll(".tab").forEach((t) =>
  t.addEventListener("click", () => {
    document.querySelectorAll(".tab").forEach((x) => x.classList.remove("selected"));
    t.classList.add("selected");
    document.querySelectorAll(".view").forEach((v) => v.classList.add("hidden"));
    $(`view-${t.dataset.view}`).classList.remove("hidden");
    if (t.dataset.view === "status") fillStatus();
    if (t.dataset.view === "peers") renderDirectory();
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

/* ------------------------------- live calls ------------------------------ */
/* Signaling: sealed envelopes over the assist path.
     caller -> ring   {t:"offer",  sdp}    (offer rides the ring)
     callee -> accept (plain)  +  payload {t:"answer", sdp}
     both   -> payload {t:"ice", candidate}
     either -> hangup / decline
   Media: direct WebRTC audio (DTLS-SRTP, end-to-end encrypted). */

const RTC_CONFIG = {
  iceServers: [
    { urls: "stun:stun.l.google.com:19302" },
    { urls: "stun:stun1.l.google.com:19302" },
  ],
};

let activeCall = null; // {peer, role, pc, stream, state, started, pendingIce[], timer}
let incoming = null;   // caller number when ringing

function callStatus(text) { $("call-status").textContent = text; }

function setLiveUI(state, peer) {
  const labels = { connecting: "CONNECTING", live: "IN CALL", ending: "ENDING" };
  $("live-call").classList.toggle("hidden", state === null);
  $("incoming").classList.toggle("hidden", state !== "incoming");
  if (state === "incoming") {
    $("incoming-num").textContent = display(peer);
  } else if (state) {
    $("call-state-label").textContent = labels[state] || "CALL";
    $("live-peer").textContent = display(peer);
  }
}

function startTimer() {
  const started = Date.now();
  activeCall.timer = setInterval(() => {
    $("call-timer").textContent = mmss(Math.floor((Date.now() - started) / 1000));
  }, 1000);
}

function stopTimer() {
  if (activeCall && activeCall.timer) { clearInterval(activeCall.timer); activeCall.timer = null; }
  const t = $("call-timer");
  if (t) t.textContent = "00:00";
}

async function getMic() {
  return navigator.mediaDevices.getUserMedia({
    audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
  });
}

function wirePeerEvents(pc, peer) {
  pc.onicecandidate = (e) => {
    if (e.candidate && activeCall && activeCall.peer === peer) {
      sealToNumber(peer, { t: "ice", candidate: e.candidate.toJSON() })
        .then((blob) => dropSignal(peer, me.number, "payload", blob))
        .catch(() => {});
    }
  };
  pc.onconnectionstatechange = () => {
    if (!activeCall || activeCall.peer !== peer) return;
    if (pc.connectionState === "connected") {
      activeCall.state = "live";
      setLiveUI("live", peer);
      callStatus("peer-to-peer audio live");
      startTimer();
    } else if (pc.connectionState === "failed") {
      callStatus("connection failed — NAT blocked the direct path (assist voice notes still work)");
      endCall(false);
    }
  };
  pc.ontrack = (e) => {
    const remote = new Audio();
    remote.srcObject = e.streams[0];
    remote.autoplay = true;
    remote.id = "remote-audio";
    document.getElementById("remote-audio")?.remove();
    document.body.appendChild(remote);
  };
}

async function placeCall() {
  if (dialed.length !== 10) return;
  if (activeCall) { callStatus("line busy — hang up first"); return; }
  const peer = dialed;
  const st = callStatus;
  st(`resolving ${display(peer)}…`);
  const rec = await resolveNumber(peer);
  if (!rec) { st("not on the network (yet)"); return; }
  let stream;
  try {
    stream = await getMic();
  } catch {
    st("microphone unavailable — allow mic access to call");
    return;
  }
  const pc = new RTCPeerConnection(RTC_CONFIG);
  activeCall = { peer, role: "caller", pc, stream, state: "ringing", pendingIce: [], timer: null };
  stream.getTracks().forEach((track) => pc.addTrack(track, stream));
  wirePeerEvents(pc, peer);
  const offer = await pc.createOffer();
  await pc.setLocalDescription(offer);
  st(`ringing ${display(peer)}…`);
  const blob = await sealToNumber(peer, { t: "offer", sdp: pc.localDescription.sdp });
  const r = await dropSignal(peer, me.number, "ring", blob);
  if (r._status !== 201) { st(`could not ring: ${r.error || "?"}`); endCall(false); return; }
  setLiveUI("connecting", peer);
  setTimeout(() => {
    if (activeCall && activeCall.state === "ringing") {
      st("no answer");
      dropSignal(peer, me.number, "hangup").catch(() => {});
      endCall(false);
    }
  }, 30000);
}

async function answerCall(fromNumber, offerSdp) {
  if (activeCall) return false;
  let stream;
  try {
    stream = await getMic();
  } catch {
    await dropSignal(fromNumber, me.number, "decline");
    callStatus("missed — microphone unavailable");
    return false;
  }
  const pc = new RTCPeerConnection(RTC_CONFIG);
  activeCall = { peer: fromNumber, role: "callee", pc, stream, state: "connecting", pendingIce: [], timer: null };
  stream.getTracks().forEach((track) => pc.addTrack(track, stream));
  wirePeerEvents(pc, fromNumber);
  await pc.setRemoteDescription({ type: "offer", sdp: offerSdp });
  for (const c of activeCall.pendingIce.splice(0)) {
    try { await pc.addIceCandidate(c); } catch { /* stale candidate */ }
  }
  const answer = await pc.createAnswer();
  await pc.setLocalDescription(answer);
  await dropSignal(fromNumber, me.number, "accept");
  const blob = await sealToNumber(fromNumber, { t: "answer", sdp: pc.localDescription.sdp });
  await dropSignal(fromNumber, me.number, "payload", blob);
  setLiveUI("connecting", fromNumber);
  callStatus("connecting…");
  return true;
}

async function endCall(sendHangup = true) {
  if (!activeCall) return;
  const { peer, pc, stream, state } = activeCall;
  stopTimer();
  if (sendHangup && peer) dropSignal(peer, me.number, state === "ringing" ? "decline" : "hangup").catch(() => {});
  try { pc && pc.close(); } catch {}
  stream && stream.getTracks().forEach((t) => t.stop());
  document.getElementById("remote-audio")?.remove();
  activeCall = null;
  $("btn-mute").textContent = "Mute";
  setLiveUI(null);
  callStatus("call ended");
}

$("btn-hangup").addEventListener("click", () => endCall(true));
$("btn-mute").addEventListener("click", () => {
  if (!activeCall || !activeCall.stream) return;
  const track = activeCall.stream.getAudioTracks()[0];
  if (!track) return;
  track.enabled = !track.enabled;
  $("btn-mute").textContent = track.enabled ? "Mute" : "Unmute";
});

$("btn-accept").addEventListener("click", async () => {
  if (!incoming) return;
  const from = incoming;
  incoming = null;
  setLiveUI(null);
  const ok = await answerCall(from, $("incoming-num").dataset.offer);
  if (!ok) callStatus("could not answer");
});
$("btn-decline").addEventListener("click", async () => {
  if (incoming) await dropSignal(incoming, me.number, "decline").catch(() => {});
  incoming = null;
  setLiveUI(null);
  callStatus("declined");
});

/* voice notes: MediaRecorder → sealed envelope → mail (the assist fallback) */
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
      if (blob.size > 1_100_000) { callStatus("voice note too long (max ~60s on assist path)"); return; }
      if (activeCall) {
        const buf = new Uint8Array(await blob.arrayBuffer());
        const sealed = await sealToNumber(activeCall.peer, { kind: "voice", mime: blob.type, data: b64(buf) });
        const r = await dropMail(activeCall.peer, me.number, sealed, "voice", 3 * 24 * 3600);
        callStatus(r._status === 201 ? "voice note delivered" : "delivery failed");
        addVoiceNote(URL.createObjectURL(blob), "out");
      }
      recording = null;
    };
    recording.start();
    $("btn-rec").textContent = "■ Stop";
  } catch {
    callStatus("microphone unavailable");
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

/* --------------------------------- peers --------------------------------- */

$("btn-refresh-peers").addEventListener("click", renderDirectory);

async function renderDirectory() {
  const list = $("peer-list");
  list.innerHTML = `<div class="conv glass"><div class="preview">reading directory…</div></div>`;
  try {
    const r = await directory();
    const rows = (r.online || []).filter((p) => p.number !== me.number);
    if (!rows.length) {
      list.innerHTML = `<div class="conv glass"><div class="num">nobody online</div><div class="preview">Open this app on a second device, claim its number, and it appears here within a minute.</div></div>`;
      return;
    }
    list.innerHTML = "";
    for (const p of rows) {
      const div = document.createElement("div");
      div.className = "conv glass";
      div.innerHTML = `<div><div class="num">${p.display_name ? `${p.display_name.replace(/</g, "&lt;")} · ` : ""}${display(p.number)}</div><div class="preview mono">${(p.fingerprint || "").replace(/</g, "&lt;")}</div></div><div class="when">online</div>`;
      div.addEventListener("click", () => {
        dialed = p.number;
        $("dial-num").textContent = display(dialed);
        document.querySelector('.tab[data-view="dialer"]').click();
        placeCall();
      });
      list.appendChild(div);
    }
  } catch {
    list.innerHTML = `<div class="conv glass"><div class="preview">directory unreachable</div></div>`;
  }
}

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

async function pollSignals() {
  if (!me) return;
  try {
    const m = await listSignals(me.number);
    for (const env of m.envelopes || []) {
      const picked = await pickSignal(me.number, env.id);
      if (!picked.envelope) continue;
      let inner = null;
      try {
        inner = JSON.parse(new TextDecoder().decode(unseal(unb64(picked.envelope.blob))));
      } catch { inner = null; }
      handleSignal(picked.envelope, inner);
    }
  } catch { /* retry next tick */ }
}

function handleEnvelope(env, inner) {
  if (inner.kind === "message") {
    const c = chats();
    c[env.from] = c[env.from] || [];
    c[env.from].push({ dir: "in", text: inner.text, ts: inner.ts || Date.now() });
    saveChats(c);
    if (openPeer === env.from) renderBubbles(); else renderConvs();
  } else if (inner.kind === "voice") {
    if (activeCall && activeCall.peer === env.from) {
      const bin = Uint8Array.from(atob(inner.data), (c) => c.charCodeAt(0));
      addVoiceNote(URL.createObjectURL(new Blob([bin], { type: inner.mime })), "in");
    }
  }
}

function handleSignal(env, inner) {
  const from = env.from;
  if (!activeCall && incoming !== from) {
    // only the newest ring matters
  }
  switch (env.signal) {
    case "ring": {
      if (activeCall) { dropSignal(from, me.number, "decline").catch(() => {}); return; }
      if (!inner || inner.t !== "offer" || !inner.sdp) { dropSignal(from, me.number, "decline").catch(() => {}); return; }
      incoming = from;
      $("incoming-num").dataset.offer = inner.sdp;
      setLiveUI("incoming", from);
      callStatus("");
      break;
    }
    case "accept": {
      if (activeCall && activeCall.peer === from && activeCall.role === "caller" && activeCall.state === "ringing") {
        activeCall.state = "connecting";
        setLiveUI("connecting", from);
        callStatus("connecting…");
      }
      break;
    }
    case "payload": {
      if (!inner || !activeCall || activeCall.peer !== from) return;
      if (inner.t === "answer" && activeCall.role === "caller") {
        activeCall.pc.setRemoteDescription({ type: "answer", sdp: inner.sdp }).then(async () => {
          for (const c of activeCall.pendingIce.splice(0)) {
            try { await activeCall.pc.addIceCandidate(c); } catch { /* stale */ }
          }
        }).catch(() => { callStatus("handshake failed"); endCall(false); });
      } else if (inner.t === "ice" && inner.candidate) {
        if (activeCall.pc.remoteDescription && activeCall.pc.remoteDescription.type) {
          activeCall.pc.addIceCandidate(inner.candidate).catch(() => {});
        } else {
          activeCall.pendingIce.push(inner.candidate);
        }
      }
      break;
    }
    case "decline": {
      if (activeCall && activeCall.peer === from) {
        callStatus("call declined");
        endCall(false);
      }
      break;
    }
    case "hangup": {
      if (activeCall && activeCall.peer === from) {
        callStatus("peer hung up");
        endCall(false);
      } else if (incoming === from) {
        incoming = null;
        setLiveUI(null);
      }
      break;
    }
  }
}

function pollLoop() {
  pollOnce();
  pollSignals();
  setInterval(pollOnce, 8000);
  setInterval(pollSignals, 2500);          // call setup needs snappier signaling
  setInterval(() => heartbeat(me.number, fingerprint(me.pk), me.name || "peer").catch(() => {}), 60000);
}

/* --------------------------------- boot ---------------------------------- */

me = loadIdentity();
if (me) enterApp();
else $("onboarding").classList.remove("hidden");
