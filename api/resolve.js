// GET /api/resolve?name=<handle | handle.me.ae | handle.ae | 5001234567>
//
// AetherNet has no DNS. A name resolves to the registry record that binds
// it to an Ed25519 identity — the record itself is the authority, served
// from the GitHub registry (raw, CDN-cached). Browsers and apps may also
// read raw.githubusercontent.com directly; this endpoint exists so clients
// get one stable, CORS-enabled, cache-friendly entry point.

import { rawGet, REPO } from "./_lib/gh.js";
import { route, send } from "./_lib/util.js";
import { NUMBER_RE, HANDLE_RE, TIERS } from "./_lib/derive.js";

export default route(async (req, res) => {
  const url = new URL(req.url, "http://x");
  const name = (url.searchParams.get("name") || "").trim().toLowerCase();
  if (!name) return send(res, 400, { error: "missing ?name=" });

  // Number form — direct record lookup.
  if (NUMBER_RE.test(name)) {
    const prefix = name.slice(0, 3);
    if (!(prefix in TIERS)) return send(res, 404, { found: false, error: "unknown tier prefix" });
    const buf = await rawGet(`registry/${prefix}/${name}.json`);
    if (!buf) return send(res, 404, { found: false, name });
    return send(res, 200, { found: true, name, record: JSON.parse(buf.toString("utf8")), source: `raw.githubusercontent.com/${REPO}` });
  }

  // Handle form: "genesis", "genesis.me.ae" or "genesis.ae".
  let handle = name;
  for (const suffix of [".me.ae", ".ae"]) {
    if (handle.endsWith(suffix)) { handle = handle.slice(0, -suffix.length); break; }
  }
  if (!HANDLE_RE.test(handle)) {
    return send(res, 400, { error: "name is not a handle, .ae domain, or ten-digit number" });
  }
  const hbuf = await rawGet(`registry/handles/${handle}.json`);
  if (!hbuf) return send(res, 404, { found: false, name: handle });
  const hrec = JSON.parse(hbuf.toString("utf8"));
  const number = hrec.number;
  let record = null;
  if (typeof number === "string" && NUMBER_RE.test(number)) {
    const prefix = number.slice(0, 3);
    if (prefix in TIERS) {
      const nbuf = await rawGet(`registry/${prefix}/${number}.json`);
      if (nbuf) record = JSON.parse(nbuf.toString("utf8"));
    }
  }
  send(res, 200, {
    found: true,
    name: handle,
    domain: `${handle}${record && record.tier === "standard" ? ".me.ae" : ".ae"}`,
    handleRecord: hrec,
    record,
    source: `raw.githubusercontent.com/${REPO}`,
  });
});
