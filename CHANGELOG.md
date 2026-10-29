

## [0.2.0] — 2026-10-29

### Added
- Assist layer: serverless functions (`api/`) over GitHub storage — health,
  resolve, atomic claim, encrypted temporary mail, call signaling, .ae site
  serving + atomic site publish, manual sweep. No backend holds keys.
- Sandbox VPS mode: `node api/sandbox-server.js` runs the identical API on
  your own machine (`docs/assist/ASSIST-LAYER.md`).
- Web client (`web/`): liquid glass UI, icy-blue accent — onboarding with
  byte-identical number derivation (vendored @noble), dialer, E2E chats via
  assist mail, .ae site browser, one-commit site publishing.
- Android: Phone tab with dialer + assist call signaling; Sites tab now
  browses live .ae sites; Settings gained profile fields (display name,
  email pointer), assist endpoint config + health test, and tokenless
  "claim via assist"; Chats gained a mesh/assist path switch; incoming
  assist mail lands in the normal conversation store (AssistPoller).
- Registry records: optional `display_name` and `email_pointer` (schema 1
  unchanged; verifier tolerates the extra keys).
- Envelope v1 interop test: JS-sealed envelope decrypted by raw libsodium
  (`tests/web_crypto_e2e.mjs` + `tests/unseal_py.py`).

### Changed
- Android tabs: PHONE / CHATS / SITES / SETTINGS (Numbers folded into
  Settings; profile now part of identity).
