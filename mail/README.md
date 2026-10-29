# AetherNet Mail — temporary encrypted envelopes

This directory is the transient mailbox of the assist layer. Every file is
an opaque ciphertext envelope addressed to a ten-digit AetherNet number:

    mail/<box>/<exp>-<id>.json

- `box` — the recipient number (500/700/900 + 7 derived digits)
- `exp` — unix seconds after which the envelope is invalid
- body  — { id, box, from, kind, exp, bytes, created_at, blob }
  where `blob` is base64: [1][ephXpk 32][nonce 24][XChaCha20-Poly1305 ct]

**Nothing here is readable by the assist endpoint, GitHub, or anyone who is
not the holder of the recipient key.** The combiner is libsodium's
crypto_box_beforenm; the proof lives in tests/web_crypto_e2e.mjs.

Envelopes are deleted on pickup, swept on read, and expire after at most
7 days. Boxes hold at most 48 live envelopes. There are no scheduled jobs —
expired mail is dropped opportunistically whenever it is touched.
