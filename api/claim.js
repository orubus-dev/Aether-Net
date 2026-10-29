// POST /api/claim — atomic registry claim, no GitHub token required on the
// device. The endpoint validates the record against the exact derivation
// rule (BLAKE2b-256(pubkey || LE64(counter))) and then performs a
// create-only write through the GitHub Contents API. GitHub rejects a
// create when the path exists, so two devices racing for the same number
// can never both win — the loser bumps its counter and re-derives.
//
// Body: a full registry record (schema 1):
//   { number, tier, pubkey, counter, created_at, schema,
//     handle?, domain?, display_name?, email_pointer? }
//
// 201 created · 409 already taken · 400 invalid record · 503 no token

import { ghCreate, requireToken } from "./_lib/gh.js";
import { route, send, readJson } from "./_lib/util.js";
import { validateRecord } from "./_lib/derive.js";

export default route(async (req, res) => {
  if (req.method !== "POST") return send(res, 405, { error: "POST only" });
  requireToken();

  const rec = await readJson(req);
  const v = validateRecord(rec);
  if (!v.ok) return send(res, 400, { error: v.error });

  const path = `registry/${v.prefix}/${rec.number}.json`;
  const claims = [];
  const claimNumber = ghCreate(
    path,
    Buffer.from(JSON.stringify(rec, null, 2)).toString("base64"),
    `AetherNet claim: ${rec.number}${rec.handle ? ` (${rec.handle})` : ""}`
  );

  // If a handle is present, also claim registry/handles/<handle>.json so
  // name lookups find the number. Both writes are atomic creates; if the
  // handle exists but points at a different number, the whole claim aborts.
  const results = await Promise.all([claimNumber]);
  if (!results[0].ok) {
    return send(res, 409, { error: "number already claimed", path });
  }

  if (rec.handle) {
    const hpath = `registry/handles/${rec.handle}.json`;
    const hrec = {
      handle: rec.handle,
      number: rec.number,
      domain: rec.domain || null,
      created_at: rec.created_at,
      schema: 1,
    };
    const hr = await ghCreate(
      hpath,
      Buffer.from(JSON.stringify(hrec, null, 2)).toString("base64"),
      `AetherNet handle claim: ${rec.handle} -> ${rec.number}`
    );
    if (!hr.ok) {
      // Number was created but handle is taken: surface the conflict
      // honestly — the number claim stands (it derives from the key), the
      // caller retries with a different handle.
      return send(res, 409, {
        error: "handle already claimed",
        handle: rec.handle,
        numberClaimed: true,
        path,
      });
    }
    claims.push(hpath);
  }

  send(res, 201, {
    ok: true,
    path,
    claims: [path, ...claims],
    number: rec.number,
    domain: rec.domain || `${rec.handle || ""}${v.prefix === "500" ? ".me.ae" : ".ae"}`,
  });
});
