// POST /api/sweep — delete every expired envelope in mail/.
// Manual hygiene: run from a workflow_dispatch or a curl. There are NO
// scheduled jobs in AetherNet tooling by design; expired envelopes are
// already invalid on read and swept opportunistically during drops.

import { ghList, ghDelete } from "./_lib/gh.js";
import { route, send } from "./_lib/util.js";

export default route(async (req, res) => {
  if (req.method !== "POST") return send(res, 405, { error: "POST only" });
  const boxes = (await ghList("mail")) || [];
  const now = Math.floor(Date.now() / 1000);
  let scanned = 0;
  let deleted = 0;
  for (const b of boxes) {
    if (b.type !== "dir") continue;
    const files = (await ghList(`mail/${b.name}`)) || [];
    for (const f of files) {
      const p = /^(\d+)-(.+)\.json$/.exec(f.name);
      if (!p) continue;
      scanned++;
      if (Number(p[1]) < now) {
        await ghDelete(
          `mail/${b.name}/${f.name}`,
          f.sha,
          `AetherNet mail: sweep expired ${p[2]}`
        ).catch(() => {});
        deleted++;
      }
    }
  }
  send(res, 200, { ok: true, boxes: boxes.filter((b) => b.type === "dir").length, scanned, deleted });
});
