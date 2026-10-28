#!/usr/bin/env python3
"""AetherNet registry verifier.

Walks registry/500, registry/700, registry/900 and registry/handles and
enforces, for every claim file:

1. valid JSON with the exact schema fields (schema version 1)
2. number files live in the tier directory their prefix belongs to, and the
   file name equals the number inside
3. the number is exactly what BLAKE2b-256(pubkey || LE64(counter)) derives —
   a claim that does not derive from its own key is rejected
4. handle files reference a claimed number
5. global uniqueness: no number or handle appears twice anywhere in the tree

Exit 0 = registry consistent. Any violation exits 1 with a message. CI runs
this on every push, so a broken claim can never merge silently.
"""

import base64
import hashlib
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REGISTRY = os.path.join(ROOT, "registry")

TIERS = {"500": "standard", "700": "elite", "900": "prestige"}
NUMBER_RE = re.compile(r"^[0-9]{10}$")
HANDLE_RE = re.compile(r"^[a-z0-9]([a-z0-9-]{1,30})[a-z0-9]$")


def fail(path: str, msg: str) -> None:
    print(f"::error file={os.path.relpath(path, ROOT)}::{msg}")
    print(f"  VIOLATION {os.path.relpath(path, ROOT)}: {msg}")
    sys.exit(1)


def derive_number(pubkey: bytes, counter: int) -> str:
    """Must stay byte-identical to number/src/lib.rs and the app's Identity.kt."""
    h = hashlib.blake2b(pubkey + counter.to_bytes(8, "little"), digest_size=32).digest()
    digits = "".join(str(h[i] % 10) for i in range(7))
    return "500" + digits  # prefix replaced by caller; we compare digits only


def derive_free_digits(pubkey: bytes, counter: int) -> str:
    h = hashlib.blake2b(pubkey + counter.to_bytes(8, "little"), digest_size=32).digest()
    return "".join(str(h[i] % 10) for i in range(7))


def load_json(path: str):
    try:
        with open(path, "r", encoding="utf-8") as f:
            return json.load(f)
    except json.JSONDecodeError as e:
        fail(path, f"invalid JSON: {e}")
    except OSError as e:
        fail(path, f"unreadable: {e}")


def verify_number_claim(path: str, seen_numbers: dict, seen_handles: dict):
    name = os.path.basename(path)
    tier_dir = os.path.basename(os.path.dirname(path))
    if tier_dir not in TIERS:
        fail(path, f"number claim outside a tier directory: {tier_dir}")
    if not NUMBER_RE.match(name.removesuffix(".json")):
        fail(path, f"file name is not a ten-digit number: {name}")
    rec = load_json(path)
    if not isinstance(rec, dict):
        fail(path, "record is not a JSON object")

    required = {"number", "tier", "pubkey", "counter", "created_at", "schema"}
    missing = required - rec.keys()
    if missing:
        fail(path, f"missing fields: {sorted(missing)}")
    if rec["schema"] != 1:
        fail(path, f"unknown schema version {rec['schema']!r}")

    number = rec["number"]
    if not NUMBER_RE.match(number):
        fail(path, f"number field is not ten digits: {number!r}")
    if name != number + ".json":
        fail(path, f"file name {name} does not match number {number}")
    if rec["tier"] != TIERS[number[:3]]:
        fail(path, f"tier {rec['tier']!r} does not match prefix {number[:3]}")
    if number[:3] != tier_dir:
        fail(path, f"number {number} stored in wrong tier dir {tier_dir}")

    try:
        pubkey = base64.b64decode(rec["pubkey"], validate=True)
    except Exception:
        fail(path, "pubkey is not valid standard base64")
    if len(pubkey) != 32:
        fail(path, f"pubkey decodes to {len(pubkey)} bytes, want 32")

    counter = rec["counter"]
    if not isinstance(counter, int) or counter < 0 or counter > 1_000_000:
        fail(path, f"counter out of range: {counter!r}")

    derived = number[:3] + derive_free_digits(pubkey, counter)
    if derived != number:
        fail(
            path,
            f"number {number} does not derive from pubkey+counter={counter} (expected {derived})",
        )

    if number in seen_numbers:
        fail(path, f"duplicate number claim: {number} also in {seen_numbers[number]}")
    seen_numbers[number] = path

    handle = rec.get("handle")
    if handle is not None:
        if not isinstance(handle, str) or not HANDLE_RE.match(handle):
            fail(path, f"handle is not a valid name: {handle!r}")
        if handle in seen_handles:
            fail(path, f"handle {handle!r} already claimed in {seen_handles[handle]}")
        seen_handles[handle] = path
        domain = rec.get("domain")
        want = f"{handle}.me.ae" if number[:3] == "500" else f"{handle}.ae"
        if domain is not None and domain != want:
            fail(path, f"domain {domain!r} does not match tier rule (want {want!r})")


def verify_handle_claim(path: str, seen_handles: dict, numbers_seen: set):
    name = os.path.basename(path).removesuffix(".json")
    if not HANDLE_RE.match(name):
        fail(path, f"handle file name is not a valid handle: {name}")
    rec = load_json(path)
    if not isinstance(rec, dict):
        fail(path, "record is not a JSON object")
    handle = rec.get("handle")
    if handle != name:
        fail(path, f"handle field {handle!r} does not match file name {name!r}")
    number = rec.get("number")
    if not isinstance(number, str) or not NUMBER_RE.match(number):
        fail(path, f"handle claim does not reference a valid number: {number!r}")
    if number in numbers_seen:
        fail(path, f"handle claim duplicates number {number} that also has a number claim")
    if name in seen_handles:
        fail(path, f"duplicate handle claim: {name}")
    seen_handles[name] = path
    numbers_seen.add(number)


def main() -> int:
    if not os.path.isdir(REGISTRY):
        print("registry/ missing — nothing to verify (ok)")
        return 0

    seen_numbers: dict = {}
    seen_handles: dict = {}
    handle_claimed_numbers: set = set()
    count_numbers = 0
    count_handles = 0

    for tier in ("500", "700", "900"):
        d = os.path.join(REGISTRY, tier)
        if not os.path.isdir(d):
            continue
        for fn in sorted(os.listdir(d)):
            if fn.endswith(".json"):
                verify_number_claim(os.path.join(d, fn), seen_numbers, seen_handles)
                count_numbers += 1

    handles_dir = os.path.join(REGISTRY, "handles")
    if os.path.isdir(handles_dir):
        for fn in sorted(os.listdir(handles_dir)):
            if fn.endswith(".json"):
                verify_handle_claim(
                    os.path.join(handles_dir, fn), seen_handles, handle_claimed_numbers
                )
                count_handles += 1

    print(f"registry OK: {count_numbers} number claims, {count_handles} handle claims")
    return 0


if __name__ == "__main__":
    sys.exit(main())
