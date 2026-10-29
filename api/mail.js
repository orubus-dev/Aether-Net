// AetherNet mail — temporary encrypted envelopes, "GitHub as the VPS".
//
// The assist endpoint never sees keys or plaintext. A sender drops a
// ciphertext blob addressed to a 10-digit number; the recipient polls and
// picks it up. GitHub stores the file temporarily; the endpoint deletes it
// on pickup. Everything in mail/ is opaque XChaCha20-Poly1305 output.
//
// Layout: mail/<box>/<exp>-<id>.json
//   box   = recipient number (10 digits)
//   exp   = unix seconds after which the envelope is invalid
//   id    = random id
//   body  = { id, box, from, kind, exp, bytes, created_at, blob }
//           blob = base64 ciphertext sealed to the recipient's pubkey
//
// GET  /api/mail?box=<number>            → list (metadata only, no blobs)
// GET  /api/mail?box=<number>&id=<id>    → pickup: fetch + delete (keep=1 to spare)
// POST /api/mail {box, from, blob, ttl, kind} → 201 {id, exp}
// DELETE /api/mail?box=<number>&id=<id>  → delete one
// DELETE /api/mail?box=<number>          → purge box
//
// Limits: MAIL_BOX_MAX envelopes per box, MAIL_BLOB_MAX base64 bytes,
// ttl clamped to MAIL_TTL_MAX. Expired envelopes are deleted opportunistically
// on every list/pickup (sweep-on-read; no scheduled jobs anywhere).

import {
  ghList,
  ghGet,
  ghCreate,
  ghDelete,
  requireToken,
  MAIL_TTL_MAX,
  MAIL_BOX_MAX,
  MAIL_BLOB_MAX,
} from "./_lib/gh.js";
import { route, send, readJson } from "./_lib/util.js";

const NUMBER = /^[0-9]{10}$/;

function parseName(name) {
  const m = /^(\d{10})-(\d+)-([0-9a-f-]+)\.json$/.exec(name);
  return m && { exp: Number(m[1]), id: m[3], full: name };
}

async function dropExpired(box, entries) {
  const now = Math.floor(Date.now() / 1000);
  await Promise.all(
    entries
      .filter((e) => e.exp && e.exp < now)
      .map((e) =>
        ghDelete(`mail/${box}/${e.full}`, e.sha, `AetherNet mail: sweep expired ${e.id}`).catch(
          () => {}
        )
      )
  );
  return entries.filter((e) => e.exp && e.exp >= now);
}

export default route(async (req, res) => {
  const url = new URL(req.url, "http://x");
  const box = url.searchParams.get("box") || "";
  const id = url.searchParams.get("id");

  if (req.method === "POST") {
    requireToken();
    const body = await readJson(req);
    if (!NUMBER.test(String(body.box || "")))
      return send(res, 400, { error: "box must be a ten-digit number" });
    if (typeof body.blob !== "string" || !body.blob.length)
      return send(res, 400, { error: "blob (base64 ciphertext) required" });
    if (body.blob.length > MAIL_BLOB_MAX)
      return send(res, 413, { error: `blob exceeds ${MAIL_BLOB_MAX} bytes` });
    const ttl = Math.min(Number(body.ttl) || 3 * 24 * 3600, MAIL_TTL_MAX);
    const now = Math.floor(Date.now() / 1000);
    const exp = now + ttl;

    const existing = (await ghList(`mail/${body.box}`)) || [];
    const live = [];
    for (const f of existing) {
      const p = parseName(f.name);
      if (!p) continue;
      if (p.exp < now) {
        await ghDelete(
          `mail/${body.box}/${f.name}`,
          f.sha,
          `AetherNet mail: sweep expired ${p.id}`
        ).catch(() => {});
      } else live.push(p);
    }
    if (live.length >= MAIL_BOX_MAX) {
      return send(res, 429, {
        error: `box ${body.box} is full (${MAIL_BOX_MAX} live envelopes) — recipient must pick up`,
      });
    }

    const idn = crypto.randomUUID();
    const env = {
      id: idn,
      box: body.box,
      from: String(body.from || "unknown"),
      kind: String(body.kind || "message"),
      exp,
      bytes: body.blob.length,
      created_at: new Date().toISOString(),
      blob: body.blob,
    };
    const path = `mail/${body.box}/${exp}-${idn}.json`;
    const r = await ghCreate(
      path,
      Buffer.from(JSON.stringify(env)).toString("base64"),
      `AetherNet mail: ${env.kind} -> ${body.box}`
    );
    if (!r.ok) return send(res, 409, { error: "drop raced, retry" });
    return send(res, 201, { ok: true, id: idn, exp, path: "accepted" });
  }

  if (req.method === "GET") {
    if (!NUMBER.test(box)) return send(res, 400, { error: "box must be a ten-digit number" });
    if (id) {
      const files = (await ghList(`mail/${box}`)) || [];
      const entry = files.find((f) => parseName(f.name) && parseName(f.name).id === id);
      if (!entry) return send(res, 404, { error: "no such envelope" });
      const p = parseName(entry.name);
      const got = await ghGet(`mail/${box}/${entry.name}`);
      if (!got) return send(res, 404, { error: "envelope vanished" });
      const env = JSON.parse(got.raw.toString("utf8"));
      const now = Math.floor(Date.now() / 1000);
      if (env.exp < now) {
        await ghDelete(
          `mail/${box}/${entry.name}`,
          entry.sha,
          `AetherNet mail: sweep expired ${env.id}`
        ).catch(() => {});
        return send(res, 410, { error: "envelope expired", id });
      }
      if (url.searchParams.get("keep") !== "1") {
        await ghDelete(
          `mail/${box}/${entry.name}`,
          entry.sha,
          `AetherNet mail: pickup ${env.id}`
        ).catch(() => {});
      }
      return send(res, 200, { envelope: env });
    }
    // list
    const files = (await ghList(`mail/${box}`)) || [];
    const now = Math.floor(Date.now() / 1000);
    const out = [];
    for (const f of files) {
      const p = parseName(f.name);
      if (!p) continue;
      if (p.exp < now) continue; // swept lazily on next write
      out.push({ id: p.id, exp: p.exp, bytes: f.size });
    }
    return send(res, 200, { box, envelopes: out, live: out.length });
  }

  if (req.method === "DELETE") {
    requireToken();
    if (!NUMBER.test(box)) return send(res, 400, { error: "box must be a ten-digit number" });
    const files = (await ghList(`mail/${box}`)) || [];
    let deleted = 0;
    for (const f of files) {
      const p = parseName(f.name);
      if (!p) continue;
      if (id && p.id !== id) continue;
      await ghDelete(`mail/${box}/${f.name}`, f.sha, `AetherNet mail: delete ${p.id}`).catch(
        () => {}
      );
      deleted++;
    }
    return send(res, 200, { ok: true, deleted });
  }

  send(res, 405, { error: "GET, POST or DELETE" });
});
