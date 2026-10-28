package net.aethernet.app

import com.goterl.lazysodium.LazySodiumAndroid
import com.goterl.lazysodium.SodiumAndroid
import com.goterl.lazysodium.interfaces.AEAD
import com.goterl.lazysodium.interfaces.Box
import com.goterl.lazysodium.interfaces.GenericHash
import com.goterl.lazysodium.interfaces.Sign
import java.security.MessageDigest
import java.security.SecureRandom

/**
 * AetherNet identity + crypto.
 *
 * The identity is an Ed25519 keypair generated from a 32-byte seed stored in
 * app-private storage. Everything derives from it:
 *
 *  - the AetherNet number: BLAKE2b-256(pubkey || LE64(counter)), first seven
 *    bytes mod 10 as the free digits behind the tier prefix — byte-for-byte
 *    the same derivation as `number/src/lib.rs` (Rust) and
 *    `tools/verify_registry.py` (CI).
 *  - the X25519 keypair (converted from the Ed25519 keys) used to establish
 *    per-peer shared keys via crypto_box_beforenm.
 *  - message protection: XChaCha20-Poly1305-IETF with 24-byte nonces and
 *    "AEN2"-style additional data binding both public keys.
 *
 * Only lazysodium *native* interface methods are used, with signatures
 * verified against the lazysodium source (interfaces/AEAD.java, Box.java,
 * Sign.java, GenericHash.java).
 */
object Crypto {

    private val ls: LazySodiumAndroid by lazy { LazySodiumAndroid(SodiumAndroid()) }
    private val sign: Sign.Native get() = ls as Sign.Native
    private val box: Box.Native get() = ls as Box.Native
    private val aead: AEAD.Native get() = ls as AEAD.Native
    private val hash: GenericHash.Native get() = ls as GenericHash.Native

    const val SEED_BYTES = 32
    const val PUB_BYTES = 32
    const val SK_BYTES = 64
    const val NONCE_BYTES = AEAD.XCHACHA20POLY1305_IETF_NPUBBYTES // 24
    const val MAC_BYTES = AEAD.XCHACHA20POLY1305_IETF_ABYTES      // 16

    fun randomBytes(n: Int): ByteArray = ByteArray(n).also { SecureRandom().nextBytes(it) }

    /** Ed25519 keypair from a 32-byte seed. Returns (publicKey, secretKey). */
    fun keypairFromSeed(seed: ByteArray): Pair<ByteArray, ByteArray> {
        require(seed.size == SEED_BYTES) { "seed must be 32 bytes" }
        val pk = ByteArray(PUB_BYTES)
        val sk = ByteArray(SK_BYTES)
        check(sign.cryptoSignSeedKeypair(pk, sk, seed)) { "crypto_sign_seed_keypair failed" }
        return pk to sk
    }

    /** Convert an Ed25519 public key to its Curve25519 (X25519) form. */
    fun toCurve25519Pk(edPk: ByteArray): ByteArray {
        val out = ByteArray(PUB_BYTES)
        check(sign.convertPublicKeyEd25519ToCurve25519(out, edPk)) {
            "convertPublicKeyEd25519ToCurve25519 failed"
        }
        return out
    }

    /** Convert an Ed25519 secret key to its Curve25519 (X25519) form. */
    fun toCurve25519Sk(edSk: ByteArray): ByteArray {
        val out = ByteArray(PUB_BYTES)
        check(sign.convertSecretKeyEd25519ToCurve25519(out, edSk)) {
            "convertSecretKeyEd25519ToCurve25519 failed"
        }
        return out
    }

    /**
     * Per-peer shared key (crypto_box_beforenm = X25519 + HSalsa20). Both
     * sides compute the same 32 bytes: beforenm(peerCurvePk, myCurveSk).
     */
    fun sharedKey(peerCurvePk: ByteArray, myCurveSk: ByteArray): ByteArray {
        val k = ByteArray(PUB_BYTES)
        check(box.cryptoBoxBeforeNm(k, peerCurvePk, myCurveSk)) { "crypto_box_beforenm failed" }
        return k
    }

    /**
     * BLAKE2b-256 keyed as specified. Byte-for-byte identical to
     * `Blake2bMac<U32>` in the Rust crate and `hashlib.blake2b(digest_size=32)`
     * in the registry verifier.
     */
    fun blake2b256(input: ByteArray): ByteArray {
        val out = ByteArray(32)
        check(hash.cryptoGenericHash(out, 32, input, input.size.toLong())) {
            "crypto_generichash failed"
        }
        return out
    }

    /** XChaCha20-Poly1305-IETF encrypt. Returns ciphertext||tag (m.len + 16). */
    fun seal(message: ByteArray, ad: ByteArray, nonce: ByteArray, key: ByteArray): ByteArray {
        require(nonce.size == NONCE_BYTES) { "nonce must be 24 bytes" }
        require(key.size == 32) { "key must be 32 bytes" }
        val cipher = ByteArray(message.size + MAC_BYTES)
        val cipherLen = longArrayOf(0)
        check(
            aead.cryptoAeadXChaCha20Poly1305IetfEncrypt(
                cipher, cipherLen,
                message, message.size.toLong(),
                ad, ad.size.toLong(),
                null, nonce, key
            )
        ) { "xchacha20poly1305ietf encrypt failed" }
        return cipher
    }

    /** XChaCha20-Poly1305-IETF decrypt. Returns null on any tamper/failure. */
    fun unseal(cipher: ByteArray, ad: ByteArray, nonce: ByteArray, key: ByteArray): ByteArray? {
        require(nonce.size == NONCE_BYTES) { "nonce must be 24 bytes" }
        require(key.size == 32) { "key must be 32 bytes" }
        val message = ByteArray(maxOf(cipher.size - MAC_BYTES, 0))
        val messageLen = longArrayOf(0)
        val ok = aead.cryptoAeadXChaCha20Poly1305IetfDecrypt(
            message, messageLen,
            null, cipher, cipher.size.toLong(),
            ad, ad.size.toLong(),
            nonce, key
        )
        return if (ok) message else null
    }

    /** Random 24-byte nonce. */
    fun nonce(): ByteArray = randomBytes(NONCE_BYTES)
}

/**
 * The AetherNet number: ten digits, tier prefix + seven derived digits.
 * Display grouping is `NNN NNNN NNN` exactly as planned: 500/700/900 for
 * standard/elite/prestige.
 */
data class AetherNumber(val tier: Tier, val digits: String) {
    init {
        require(digits.length == 7 && digits.all { it in '0'..'9' }) { "digits must be 7" }
    }

    val plain: String get() = "${tier.prefix}$digits"
    val display: String get() = "${tier.prefix} ${digits.take(4)} ${digits.drop(4)}"

    companion object {
        /**
         * Derive from an Ed25519 public key at `counter`:
         * BLAKE2b-256(pubkey || LE64(counter)), h[0..7] % 10.
         */
        fun derive(pubKey: ByteArray, counter: Long, tier: Tier = Tier.STANDARD): AetherNumber {
            require(pubKey.size == Crypto.PUB_BYTES) { "pubkey must be 32 bytes" }
            val input = ByteArray(40)
            pubKey.copyInto(input)
            for (i in 0..7) input[32 + i] = ((counter ushr (8 * i)) and 0xFF).toByte()
            val h = Crypto.blake2b256(input)
            val digits = h.take(7).joinToString("") { (it.toInt() and 0xFF).mod(10).toString() }
            return AetherNumber(tier, digits)
        }

        fun parse(s: String): AetherNumber? {
            val plain = s.filter { it.isDigit() }
            if (plain.length != 10) return null
            val prefix = plain.take(3).toIntOrNull() ?: return null
            val tier = Tier.fromPrefix(prefix) ?: return null
            return AetherNumber(tier, plain.drop(3))
        }
    }
}

/** Tier encoded by the number prefix. */
enum class Tier(val prefix: Int, val label: String, val domainSuffix: String) {
    STANDARD(500, "Standard", "me.ae"),
    ELITE(700, "Elite", "ae"),
    PRESTIGE(900, "Prestige", "ae");

    companion object {
        fun fromPrefix(prefix: Int): Tier? = entries.firstOrNull { it.prefix == prefix }
    }
}

/** The on-device identity: seed, keys, derived number, handle. */
class Identity(
    val seed: ByteArray,
    val publicKey: ByteArray,
    val secretKey: ByteArray,
    val counter: Long,
    val tier: Tier,
) {
    val number: AetherNumber by lazy { AetherNumber.derive(publicKey, counter, tier) }
    val curvePk: ByteArray by lazy { Crypto.toCurve25519Pk(publicKey) }
    val curveSk: ByteArray by lazy { Crypto.toCurve25519Sk(secretKey) }

    /** 16-hex fingerprint of the public key, shown in the UI. */
    val fingerprint: String by lazy {
        val d = MessageDigest.getInstance("SHA-256").digest(publicKey)
        d.take(8).joinToString("") { "%02x".format(it) }.uppercase()
    }

    companion object {
        /** Generate a fresh identity at counter 0. */
        fun generate(tier: Tier): Identity {
            val seed = Crypto.randomBytes(Crypto.SEED_BYTES)
            val (pk, sk) = Crypto.keypairFromSeed(seed)
            return Identity(seed, pk, sk, 0, tier)
        }

        /** Rebuild from stored bytes (seed + counter + tier). */
        fun fromStored(seed: ByteArray, counter: Long, tier: Tier): Identity {
            val (pk, sk) = Crypto.keypairFromSeed(seed)
            return Identity(seed, pk, sk, counter, tier)
        }

        /** Key for talking to `peerPublicKey` (their Ed25519 public key). */
        fun keyToPeer(me: Identity, peerPublicKey: ByteArray): ByteArray =
            Crypto.sharedKey(Crypto.toCurve25519Pk(peerPublicKey), me.curveSk)
    }
}
