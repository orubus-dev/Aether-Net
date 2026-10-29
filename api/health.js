// GET /api/health — liveness + configuration of this assist endpoint.
// Never touches secrets; safe to poll from apps and browsers.

import { REPO, TOKEN } from "./_lib/gh.js";
import { route, send } from "./_lib/util.js";

export default route(async (req, res) => {
  send(res, 200, {
    ok: true,
    service: "aethernet-assist",
    version: 1,
    repo: REPO,
    capabilities: {
      claim: true, // atomic number/handle claims via GitHub Contents API
      resolve: true, // .ae name + number resolution
      mail: true, // temporary encrypted envelopes (ciphertext only)
      signal: true, // call signaling envelopes
      sites: true, // .ae site package serving
      writes: Boolean(TOKEN), // writes need GITHUB_TOKEN on the endpoint
    },
    ts: new Date().toISOString(),
  });
});
