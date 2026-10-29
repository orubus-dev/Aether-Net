package net.aethernet.app.net

import net.aethernet.app.Crypto

/**
 * Assist-layer crypto: the exact envelope the web client (web/aether.js)
 * speaks, built only from primitives this app already uses.
 *
 * Envelope v1 layout: [version 1][ephemeral X25519 pk 32][nonce 24][ct+tag]
 *
 *   shared = crypto_box_beforenm(recipientX25519Pk, ephemeralX25519Sk)
 *            — libsodium's X25519 + HSalsa20 combiner; symmetric, so both
 *            sides compute the same 32 bytes from mirrored inputs
 *   ct     = XChaCha20-Poly1305-IETF(shared, nonce, aad = ephPk || recipPk)
 *
 * The web side implements the same combiner exactly (validated against
 * libsodium vectors in tests/web_crypto_e2e.mjs), so Android mail and web
 * mail decrypt each other with no shared native dependency.
 */
object AssistCrypto {

    /** Seal to a recipient's X25519 public key. Envelope v1, random ephemeral key. */
    fun seal(recipXpk: ByteArray, plaintext: ByteArray): ByteArray {
        require(recipXpk.size == 32) { "recipient X25519 pk must be 32 bytes" }

        // Ephemeral X25519 keypair from a throwaway Ed25519 seed (existing path).
        val edSeed = Crypto.randomBytes(32)
        val (edPk, edSk) = Crypto.keypairFromSeed(edSeed)
        val ephXsk = Crypto.toCurve25519Sk(edSk)
        val ephXpk = Crypto.toCurve25519Pk(edPk)

        // beforenm(recipientPk, ephemeralSk) — same combiner the recipient inverts.
        val shared = Crypto.sharedKey(recipXpk, ephXsk)

        val nonce = Crypto.nonce()
        val ad = ephXpk + recipXpk
        val ct = Crypto.seal(plaintext, ad, nonce, shared)

        val out = ByteArray(1 + 32 + 24 + ct.size)
        out[0] = 1
        System.arraycopy(ephXpk, 0, out, 1, 32)
        System.arraycopy(nonce, 0, out, 33, 24)
        System.arraycopy(ct, 0, out, 57, ct.size)
        return out
    }

    /** Open an envelope v1 addressed to me. Returns null on tamper/foreign mail. */
    fun unseal(myXsk: ByteArray, myXpk: ByteArray, envelope: ByteArray): ByteArray? {
        if (envelope.size < 58 || envelope[0] != 1.toByte()) return null
        val ephPk = envelope.copyOfRange(1, 33)
        val nonce = envelope.copyOfRange(33, 57)
        val ct = envelope.copyOfRange(57, envelope.size)
        val shared = Crypto.sharedKey(ephPk, myXsk) // beforenm(ephPk, mySk)
        val ad = ephPk + myXpk
        return Crypto.unseal(ct, ad, nonce, shared)
    }
}
