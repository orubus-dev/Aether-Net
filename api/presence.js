// AetherNet presence — the talk directory, "GitHub as the VPS".
//
// A peer heartbeat writes presence/<number>.json with a short expiry.
// The directory listing is any live record. Sweeping is lazy (on read) —
// no scheduled jobs anywhere, same discipline as mail.
//
// POST /api/presence {number, fingerprint, display_name, ttl=120}
//   → upsert, 200 {ok, exp}
// GET  /api/presence          → {online: [{number, fingerprint, display_name, seen_at}], count}
// DELETE /api/presence?number=<number> → remove own record
//
// Numbers are 10-digit AetherNet numbers; fingerprint is the hex Ed25519
// pubkey fingerprint (a display aid, not an authority — the registry is).

import { ghList, ghGet, ghUpsert, ghDelete, requireToken } from "./_lib/gh.js";
import { route, send, readJson } from "./_lib/util.js";

const NUMBER = /^[0-9]{10}$/;
const PRESENCE_TTL_DEFAULT = 120;
const PRESENCE_TTL_MAX = 600;
const PRESENCE_MAX_PEERS = 512;

function parseName(name) {
  const m = /^(\d{10})\.json$/.exec(name);
  return m && m[1];
}

export default route(async (req, res) => {
  const url = new URL(req.url, "http://x");

  if (req.method === "POST") {
    requireToken();
    const body = await readJson(req);
    const number = String(body.number || "");
    if (!NUMBER.test(number))
      return send(res, 400, { error: "number must be ten digits" });
    const fingerprint = String(body.fingerprint || "").slice(0, 128);
    if (!fingerprint) return send(res, 400, { error: "fingerprint required" });
    const display_name = String(body.display_name || "peer").slice(0, 60);
    const ttl = Math.min(Number(body.ttl) || PRESENCE_TTL_DEFAULT, PRESENCE_TTL_MAX);

    const files = (await ghList("presence")) || [];
    if (files.length > PRESENCE_MAX_PEERS) {
      return send(res, 429, { error: "directory full — sweep pending" });
    }

    const now = Math.floor(Date.now() / 1000);
    const exp = now + ttl;
    const record = {
      number,
      fingerprint,
      display_name,
      seen_at: now,
      exp,
      schema: 1,
    };
    await ghUpsert(
      `presence/${number}.json`,
      Buffer.from(JSON.stringify(record)).toString("base64"),
      `AetherNet presence: ${number}`
    );
    return send(res, 200, { ok: true, number, exp });
  }

  if (req.method === "GET") {
    const files = (await ghList("presence")) || [];
    const now = Math.floor(Date.now() / 1000);
    const online = [];
    const expired = [];
    for (const f of files) {
      const number = parseName(f.name);
      if (!number) continue;
      const got = await ghGet(`presence/${f.name}`);
      if (!got || !got.raw) {
        continue;
      }
      let rec;
      try {
        rec = JSON.parse(got.raw.toString("utf8"));
      } catch {
        continue;
      }
      if (!rec.exp || rec.exp < now) {
        expired.push({ name: f.name, sha: got.sha, number });
        continue;
      }
      online.push({
        number: rec.number,
        fingerprint: rec.fingerprint,
        display_name: rec.display_name,
        seen_at: rec.seen_at,
      });
    }
    // Lazy sweep — expired records are deleted opportunistically on read.
    await Promise.all(
      expired.map((e) =>
        ghDelete(
          `presence/${e.name}`,
          e.sha,
          `AetherNet presence: sweep ${e.number}`
        ).catch(() => {})
      )
    );
    online.sort((a, b) => a.display_name.localeCompare(b.display_name));
    return send(res, 200, { online, count: online.length });
  }

  if (req.method === "DELETE") {
    const number = url.searchParams.get("number") || "";
    if (!NUMBER.test(number))
      return send(res, 400, { error: "number must be ten digits" });
    requireToken();
    const got = await ghGet(`presence/${number}.json`);
    if (got && got.sha) {
      await ghDelete(
        `presence/${number}.json`,
        got.sha,
        `AetherNet presence: leave ${number}`
      ).catch(() => {});
    }
    return send(res, 200, { ok: true });
  }

  send(res, 405, { error: "GET, POST or DELETE" });
});
