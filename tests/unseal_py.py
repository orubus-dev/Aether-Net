#!/usr/bin/env python3
"""Unseal a JS-sealed AetherNet assist envelope with RAW LIBSODIUM (PyNaCl).

This is the compatibility proof: the Android app uses these exact bindings
through lazysodium, so if libsodium can open the web's mail, Android can.

Reads /tmp/aether_env.json {envelope, recip_xsk, recip_xpk} (all base64),
performs crypto_box_beforenm + XChaCha20-Poly1305-IETF manually, prints the
plaintext, exits 0 with PY-UNSEAL-OK on success.
"""
import base64
import json
import sys

import nacl.bindings as sodium

data = json.load(open("/tmp/aether_env.json"))
env = base64.b64decode(data["envelope"])
xsk = base64.b64decode(data["recip_xsk"])
xpk = base64.b64decode(data["recip_xpk"])

if env[0] != 1:
    print("bad version"); sys.exit(1)
eph_pk = env[1:33]
nonce = env[33:57]
ct = env[57:]

shared = sodium.crypto_box_beforenm(eph_pk, xsk)
aad = eph_pk + xpk
pt = sodium.crypto_aead_xchacha20poly1305_ietf_decrypt(ct, aad, nonce, shared)
inner = json.loads(pt.decode())
print(f"libsodium (python) decrypted from JS envelope: {inner['text']!r}")
assert inner["text"] == "hello from the web"
print("PY-UNSEAL-OK")
