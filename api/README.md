# AetherNet Assist API

Stateless serverless functions over GitHub storage. Deploy target:
**aether-net.vercel.app** (or run `sandbox-server.js` on your own box).

```
api/
├── health.js          GET  liveness + capabilities
├── resolve.js         GET  name/number/domain → registry record
├── claim.js           POST atomic number+handle claim (no device token needed)
├── mail.js            POST/GET/DELETE temporary encrypted envelopes
├── signal.js          POST/GET call signaling (short-TTL envelopes)
├── sites.js           GET/POST .ae site packages (browse + atomic publish)
├── sweep.js           POST delete expired envelopes (manual only)
├── sandbox-server.js  run the whole API on your own machine
└── _lib/              shared helpers (GitHub client, derivation, validation)
    └── _vendor/       vendored @noble crypto (blake2b — byte-identical
                      derivation with the Rust core, Kotlin app and CI)
```

## Deploy to Vercel

1. vercel.com/new → import `orubus-dev/Aether-Net` → framework: **Other** → deploy.
2. Project → Settings → Environment Variables:
   - `GITHUB_TOKEN` — fine-grained token, **contents: read+write** on `orubus-dev/Aether-Net` only
   - `GITHUB_REPO` — `orubus-dev/Aether-Net`
3. Redeploy. `https://aether-net.vercel.app/api/health` must show
   `"writes": true`.

The web client (`web/`) deploys as part of the same project — the root URL
is the app.

## Run on your own box (sandbox = VPS)

```bash
cd Aether-Net
GITHUB_TOKEN=… GITHUB_REPO=orubus-dev/Aether-Net node api/sandbox-server.js 8787
```

Same routes, same env contract, serves the web client too.

## Zero-dependency

Node ≥ 20 standard library + the vendored noble files under `_lib/_vendor/`.
No npm install anywhere in this directory.
