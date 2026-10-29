package net.aethernet.app.net

import android.util.Base64
import java.util.Locale
import org.json.JSONArray
import org.json.JSONObject
import java.io.BufferedReader
import java.io.InputStreamReader
import java.net.HttpURLConnection
import java.net.URL
import net.aethernet.app.Crypto

/**
 * Client for the AetherNet assist layer — the serverless "VPS" built on
 * GitHub storage (registry records, temporary encrypted mail) fronted by
 * stateless functions (Vercel by default, or your own sandbox running
 * api/sandbox-server.js).
 *
 * The endpoint is a dumb ciphertext pipe: it can route our envelopes but
 * cannot read them. No keys, no plaintext, no database exist anywhere on
 * that path. Default endpoint: https://aether-net.vercel.app
 */
class AssistClient(private val endpoint: String) {

    sealed class Result {
        data class Ok(val json: JSONObject, val status: Int) : Result()
        data class Err(val message: String, val status: Int) : Result()
    }

    private fun request(
        method: String,
        path: String,
        body: JSONObject? = null,
        timeoutMs: Int = 15_000,
    ): Result {
        val conn = URL(endpoint + path).openConnection() as HttpURLConnection
        return try {
            conn.requestMethod = method
            conn.setRequestProperty("Accept", "application/json")
            if (body != null) {
                conn.setRequestProperty("Content-Type", "application/json")
                conn.doOutput = true
                conn.outputStream.use { it.write(body.toString().toByteArray()) }
            }
            conn.connectTimeout = timeoutMs
            conn.readTimeout = timeoutMs
            val code = conn.responseCode
            val stream = if (code in 200..299) conn.inputStream else conn.errorStream
            val text = stream?.bufferedReader()?.use(BufferedReader::readText) ?: "{}"
            val json = runCatching { JSONObject(text) }.getOrDefault(JSONObject())
            if (code in 200..299) Result.Ok(json, code) else Result.Err(json.optString("error", "HTTP $code"), code)
        } catch (e: Exception) {
            Result.Err("assist unreachable: ${e.message}", 0)
        } finally {
            conn.disconnect()
        }
    }

    /** Liveness + capability probe. */
    fun health(): Result = request("GET", "/api/health")

    /** Resolve a number, handle or .ae domain to its registry record. */
    fun resolve(name: String): Result =
        request("GET", "/api/resolve?name=${java.net.URLEncoder.encode(name, "UTF-8")}")

    /** Claim without a personal GitHub token — the endpoint does the write. */
    fun claim(record: JSONObject): Result =
        request("POST", "/api/claim", record)

    /** Drop an encrypted envelope into a box (the recipient's number). */
    fun mailDrop(box: String, from: String, blobB64: String, kind: String, ttlSeconds: Int): Result =
        request(
            "POST", "/api/mail",
            JSONObject()
                .put("box", box)
                .put("from", from)
                .put("blob", blobB64)
                .put("kind", kind)
                .put("ttl", ttlSeconds)
        )

    /** List live envelope metadata for a box. */
    fun mailList(box: String): Result = request("GET", "/api/mail?box=$box")

    /** Pick up (and delete) one envelope. */
    fun mailPick(box: String, id: String): Result =
        request("GET", "/api/mail?box=$box&id=${java.net.URLEncoder.encode(id, "UTF-8")}")

    /** Call signaling: ring / accept / decline / hangup, short TTL. */
    fun signal(to: String, from: String, signal: String, blobB64: String): Result =
        request(
            "POST", "/api/signal",
            JSONObject()
                .put("to", to)
                .put("from", from)
                .put("signal", signal)
                .put("blob", blobB64)
                .put("ttl", 120)
        )

    /** Fetch an .ae site package file (internet-assisted browsing). */
    fun sitesGet(name: String, path: String): Result =
        request(
            "GET",
            "/api/sites?name=${java.net.URLEncoder.encode(name, "UTF-8")}" +
                "&path=${java.net.URLEncoder.encode(path, "UTF-8")}",
            timeoutMs = 20_000,
        )

    /**
     * Full assist claim flow: derive with counter retry until the registry
     * accepts the number. Returns a human-readable status line.
     */
    fun claimFlow(
        identityPubKeyB64: String,
        tierName: String,
        deriveNumber: (Long) -> net.aethernet.app.AetherNumber,
        startCounter: Long,
        handle: String?,
        displayName: String?,
        emailPointer: String?,
        onRederive: (Long) -> Unit,
    ): String {
        var attempt = startCounter
        var last = ""
        for (tries in 0 until 5) {
            val number = deriveNumber(attempt)
            val rec = JSONObject()
                .put("number", number.plain)
                .put("tier", tierName)
                .put("pubkey", identityPubKeyB64)
                .put("counter", attempt)
                .put("created_at", nowRfc3339())
                .put("schema", 1)
            if (!displayName.isNullOrEmpty()) rec.put("display_name", displayName)
            if (!emailPointer.isNullOrEmpty()) rec.put("email_pointer", emailPointer)
            if (!handle.isNullOrEmpty()) {
                rec.put("handle", handle)
                rec.put("domain", if (tierName == "standard") "$handle.me.ae" else "$handle.ae")
            }
            when (val r = claim(rec)) {
                is Result.Ok -> return "${number.display} claimed via assist"
                is Result.Err -> {
                    if (r.status == 409) {
                        last = "${number.display} taken — re-deriving"
                        attempt++
                        onRederive(attempt)
                        continue
                    }
                    return "assist claim failed: ${r.message}"
                }
            }
        }
        return last.ifEmpty { "claim failed" }
    }

    private fun nowRfc3339(): String =
        java.text.SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss'Z'", Locale.US).apply {
            timeZone = java.util.TimeZone.getTimeZone("UTC")
        }.format(java.util.Date())

    companion object {
        /**
         * Envelope seal helper: seal an arbitrary JSON payload to a registry
         * record's public key. The registry stores the Ed25519 key, so the
         * X25519 conversion happens here (same rule the recipient applies).
         */
        fun sealToRecordPubkey(recipEdPkB64: String, payload: JSONObject): String? {
            return runCatching {
                val edPk = Base64.decode(recipEdPkB64, Base64.NO_WRAP)
                val recipXpk = Crypto.toCurve25519Pk(edPk)
                val env = AssistCrypto.seal(recipXpk, payload.toString().toByteArray())
                Base64.encodeToString(env, Base64.NO_WRAP)
            }.getOrNull()
        }

        /** Envelope open helper for mail addressed to us (my Ed25519 secret key). */
        fun unsealEnvelope(myEdSk: ByteArray, myEdPk: ByteArray, blobB64: String): JSONObject? {
            return runCatching {
                val env = Base64.decode(blobB64, Base64.NO_WRAP)
                val myXsk = Crypto.toCurve25519Sk(myEdSk)
                val myXpk = Crypto.toCurve25519Pk(myEdPk)
                val pt = AssistCrypto.unseal(myXsk, myXpk, env) ?: return null
                JSONObject(String(pt, Charsets.UTF_8))
            }.getOrNull()
        }
    }
}
