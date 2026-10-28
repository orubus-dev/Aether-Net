package net.aethernet.app.net

import android.util.Base64
import org.json.JSONObject
import java.io.HttpURLConnection
import java.net.URL

/**
 * GitHub-backed AetherNet number/handle registry client.
 *
 * A claim is an atomic file create via the Contents API:
 *   PUT /repos/orubus-dev/Aether-Net/contents/registry/<tier>/<number>.json
 * GitHub rejects creates when the path already exists (422), which is exactly
 * the anti-double-allocation property the network needs: one of two racing
 * devices wins, the loser bumps its counter, re-derives and retries.
 *
 * The user supplies a GitHub fine-grained token with contents:write on
 * orubus-dev/Aether-Net in Settings. The token stays in app-private storage
 * and is sent only to api.github.com over TLS.
 */
class RegistryClient(private val token: String) {

    companion object {
        const val OWNER = "orubus-dev"
        const val REPO = "Aether-Net"
        private const val API = "https://api.github.com/repos/$OWNER/$REPO/contents/"
    }

    sealed class ClaimResult {
        /** Claim landed; [number] is now globally unique on the registry. */
        data class Claimed(val path: String, val commitSha: String) : ClaimResult()
        /** The number/handle already belongs to someone else. */
        data class AlreadyTaken(val path: String) : ClaimResult()
        /** Transport/auth problem; the message explains what to check. */
        data class Error(val message: String) : ClaimResult()
    }

    private fun put(path: String, json: JSONObject): ClaimResult {
        val body = JSONObject()
            .put("message", "AetherNet claim: $path")
            .put(
                "content",
                Base64.encodeToString(json.toString().toByteArray(), Base64.NO_WRAP)
            )
        val conn = URL(API + path).openConnection() as HttpURLConnection
        return try {
            conn.requestMethod = "PUT"
            conn.setRequestProperty("Authorization", "Bearer $token")
            conn.setRequestProperty("Accept", "application/vnd.github+json")
            conn.setRequestProperty("Content-Type", "application/json")
            conn.doOutput = true
            conn.connectTimeout = 15_000
            conn.readTimeout = 15_000
            conn.outputStream.use { it.write(body.toString().toByteArray()) }
            val code = conn.responseCode
            when {
                code == 201 -> {
                    val resp = conn.inputStream.bufferedReader().readText()
                    val sha = JSONObject(resp)
                        .getJSONObject("content")
                        .optString("sha")
                    ClaimResult.Claimed(path, sha)
                }
                code == 422 || code == 409 -> ClaimResult.AlreadyTaken(path)
                code == 401 -> ClaimResult.Error("token rejected (401) — check the token is valid and has contents:write on $OWNER/$REPO")
                code == 403 -> ClaimResult.Error("forbidden (403) — the token lacks contents:write on $OWNER/$REPO")
                else -> {
                    val err = runCatching {
                        conn.errorStream?.bufferedReader()?.readText()
                    }.getOrNull().orEmpty().take(240)
                    ClaimResult.Error("HTTP $code $err")
                }
            }
        } catch (e: Exception) {
            ClaimResult.Error("network error: ${e.message ?: e.javaClass.simpleName}")
        } finally {
            conn.disconnect()
        }
    }

    /**
     * Claim a number with its derivation counter. The record shape matches
     * registry/README.md schema 1 exactly, so the CI verifier accepts it.
     */
    fun claimNumber(
        number: String,
        tierName: String,
        pubKeyB64: String,
        counter: Long,
        handle: String?,
    ): ClaimResult {
        val o = JSONObject()
            .put("number", number)
            .put("tier", tierName)
            .put("pubkey", pubKeyB64)
            .put("counter", counter)
            .put("created_at", nowRfc3339())
            .put("schema", 1)
        if (!handle.isNullOrEmpty()) {
            o.put("handle", handle)
            o.put("domain", if (tierName == "standard") "$handle.me.ae" else "$handle.ae")
        }
        return put("registry/${number.take(3)}/$number.json", o)
    }

    /** Claim a handle, bound to an already-claimed number. */
    fun claimHandle(handle: String, number: String): ClaimResult {
        val o = JSONObject()
            .put("handle", handle)
            .put("number", number)
            .put("created_at", nowRfc3339())
            .put("schema", 1)
        return put("registry/handles/$handle.json", o)
    }

    private fun nowRfc3339(): String =
        java.text.SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss'Z'", java.util.Locale.US).apply {
            timeZone = java.util.TimeZone.getTimeZone("UTC")
        }.format(java.util.Date())
}
