package net.aethernet.app.data

import android.content.Context
import net.aethernet.app.Identity
import net.aethernet.app.Tier
import org.json.JSONObject
import java.io.File

/**
 * On-device identity persistence. The 32-byte seed is the whole identity:
 * keys re-derive from it, so only the seed + counter + tier are stored.
 *
 * v0.1.0 stores the seed in app-private storage (mode private, unencrypted).
 * Roadmap v0.2 wraps it with an Android Keystore key; noted in docs/roadmap.
 */
class IdentityStore(context: Context) {

    private val file = File(context.filesDir, "identity.json")

    fun exists(): Boolean = file.exists()

    fun save(identity: Identity, handle: String?) {
        val o = JSONObject()
            .put("seed", b64(identity.seed))
            .put("counter", identity.counter)
            .put("tier", identity.tier.name)
            .put("handle", handle ?: "")
        file.writeText(o.toString())
    }

    /** Load the identity, or null if not created yet / unreadable. */
    fun load(): Pair<Identity, String?>? {
        if (!file.exists()) return null
        return runCatching {
            val o = JSONObject(file.readText())
            val seed = ub64(o.getString("seed"))
            val identity = Identity.fromStored(seed, o.getLong("counter"), Tier.valueOf(o.getString("tier")))
            val handle = o.optString("handle").ifEmpty { null }
            identity to handle
        }.getOrNull()
    }

    /** Bump the derivation counter (registry collision retry) and persist. */
    fun updateCounter(identity: Identity, newCounter: Long, handle: String?) {
        val rebuilt = Identity.fromStored(identity.seed, newCounter, identity.tier)
        save(rebuilt, handle)
    }

    private fun b64(b: ByteArray): String =
        android.util.Base64.encodeToString(b, android.util.Base64.NO_WRAP)

    private fun ub64(s: String): ByteArray =
        android.util.Base64.decode(s, android.util.Base64.NO_WRAP)
}
