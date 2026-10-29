// AetherNet call signaling — thin alias over the encrypted mail drop.
//
// Call setup (ring / accept / decline / hangup / ICE-style payloads) rides
// the same temporary encrypted envelopes as messages, with short TTLs and
// a dedicated kind, so the assist endpoint stays a dumb ciphertext pipe.
// The payload inside the blob is end-to-end encrypted between caller and
// callee exactly like chat; live media then flows device-to-device over
// the mesh path when a route exists (L4 realtime sessions), with the
// assist path as the internet-assisted fallback channel.
//
// POST /api/signal  {to, from, blob, ttl=90, kind="signal"}  → drop
// GET  /api/signal?box=<number>                  → list signaling envelopes
// GET  /api/signal?box=<number>&id=<id>          → pickup + delete

import { ghList, ghGet, ghCreate, ghDelete, requireToken } from "./_lib/gh.js";
import { route, send, readJson } from "./_lib/util.js";

const NUMBER = /^[0-9]{10}$/;
const SIGNAL_TTL_MAX = 600; // signaling is transient by design

export default route(async (req, res) => {
  const url = new URL(req.url, "http://x");
  const box = url.searchParams.get("box") || "";
  const id = url.searchParams.get("id");

  if (req.method === "POST") {
    requireToken();
    const body = await readJson(req);
    const to = String(body.to || "");
    if (!NUMBER.test(to)) return send(res, 400, { error: "to must be a ten-digit number" });
    if (typeof body.blob !== "string" || !body.blob.length)
      return send(res, 400, { error: "blob (base64 ciphertext) required" });
    if (body.blob.length > 200_000)
      return send(res, 413, { error: "signal payload too large" });
    const ttl = Math.min(Number(body.ttl) || 90, SIGNAL_TTL_MAX);
    const exp = Math.floor(Date.now() / 1000) + ttl;
    const idn = crypto.randomUUID();
    const env = {
      id: idn,
      box: to,
      from: String(body.from || "unknown"),
      kind: "signal",
      signal: String(body.signal || "ring"), // ring|accept|decline|hangup|payload
      exp,
      bytes: body.blob.length,
      created_at: new Date().toISOString(),
      blob: body.blob,
    };
    const path = `mail/${to}/${exp}-${idn}.json`;
    const r = await ghCreate(
      path,
      Buffer.from(JSON.stringify(env)).toString("base64"),
      `AetherNet signal: ${env.signal} -> ${to}`
    );
    if (!r.ok) return send(res, 409, { error: "drop raced, retry" });
    return send(res, 201, { ok: true, id: idn, exp });
  }

  if (req.method === "GET") {
    if (!NUMBER.test(box)) return send(res, 400, { error: "box must be a ten-digit number" });
    const files = (await ghList(`mail/${box}`)) || [];
    const now = Math.floor(Date.now() / 1000);
    if (id) {
      const entry = files.find((f) => {
        const p = /^(\d+)-(.+)\.json$/.exec(f.name);
        return p && p[2] === id;
      });
      if (!entry) return send(res, 404, { error: "no such signal" });
      const got = await ghGet(`mail/${box}/${entry.name}`);
      if (!got) return send(res, 404, { error: "signal vanished" });
      const env = JSON.parse(got.raw.toString("utf8"));
      await ghDelete(
        `mail/${box}/${entry.name}`,
        entry.sha,
        `AetherNet signal: pickup ${env.id}`
      ).catch(() => {});
      if (env.exp < now) return send(res, 410, { error: "signal expired" });
      return send(res, 200, { envelope: env });
    }
    const out = [];
    for (const f of files) {
      const p = /^(\d+)-(.+)\.json$/.exec(f.name);
      if (!p || Number(p[1]) < now) continue;
      out.push({ id: p[2], exp: Number(p[1]), bytes: f.size });
    }
    return send(res, 200, { box, envelopes: out });
  }

  send(res, 405, { error: "GET or POST" });
});
