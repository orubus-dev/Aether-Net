package net.aethernet.app.net

import net.aethernet.app.Identity
import net.aethernet.app.NetGraph
import net.aethernet.app.data.MessageStore

/**
 * Background pump for the assist path. Every few seconds the app lists the
 * encrypted envelopes addressed to our number, picks them up (the endpoint
 * deletes on pickup) and unseals them locally:
 *
 *   - kind "message" → appended into the normal conversation store, so
 *     assist-delivered messages appear in the same chats as mesh-delivered
 *   - kind "signal"  → rings the Phone tab (AssistBus.call)
 *
 * All cryptography is local; the endpoint only ever relays ciphertext.
 */
object AssistPoller {

    /** One poll pass. Returns true when something new arrived. */
    fun poll(identity: Identity): Boolean {
        val me = identity
        val client = AssistClient(NetGraph.assistEndpoint)
        var fresh = false

        val list = when (val r = client.mailList(me.number.plain)) {
            is AssistClient.Result.Ok -> r.json.optJSONArray("envelopes") ?: return false
            is AssistClient.Result.Err -> return false
        }
        for (i in 0 until list.length()) {
            val id = list.getJSONObject(i).optString("id") ?: continue
            val picked = when (val r = client.mailPick(me.number.plain, id)) {
                is AssistClient.Result.Ok -> r.json.optJSONObject("envelope") ?: continue
                is AssistClient.Result.Err -> continue
            }
            val inner = AssistClient.unsealEnvelope(me.secretKey, me.publicKey, picked.optString("blob"))
                ?: continue // foreign/corrupt — dropped
            val from = picked.optString("from")
            when (picked.optString("kind")) {
                "message" -> {
                    NetGraph.messages.append(
                        from,
                        MessageStore.Msg(
                            id = "assist-" + picked.optString("id"),
                            ts = inner.optLong("ts", System.currentTimeMillis()),
                            outgoing = false,
                            text = inner.optString("text"),
                            status = MessageStore.Status.DELIVERED,
                        )
                    )
                    fresh = true
                }
                "signal" -> {
                    when (picked.optString("signal")) {
                        "ring" -> net.aethernet.app.ui.AssistBus.call.value =
                            net.aethernet.app.ui.AssistBus.CallEvent.Ring(from)
                        "decline", "hangup" -> net.aethernet.app.ui.AssistBus.call.value =
                            net.aethernet.app.ui.AssistBus.CallEvent.Idle
                        else -> {}
                    }
                    fresh = true
                }
            }
        }
        return fresh
    }

    /** Fire a ring at a number and return a status line for the UI. */
    fun ring(identity: Identity, to: String): String {
        val client = AssistClient(NetGraph.assistEndpoint)
        val blob = AssistClient.sealToRecordPubkey(
            runCatching { resolvePubkey(to) }.getOrDefault(""),
            org.json.JSONObject().put("kind", "call").put("ts", System.currentTimeMillis()),
        ) ?: return "cannot seal — peer not on registry"
        return when (val r = client.signal(to, identity.number.plain, "ring", blob)) {
            is AssistClient.Result.Ok -> "ringing ${to.chunked(3).joinToString(" ")}…"
            is AssistClient.Result.Err -> "call failed: ${r.message}"
        }
    }

    /** Answer machinery for the UI. */
    fun answer(identity: Identity, to: String, accept: Boolean): String {
        val client = AssistClient(NetGraph.assistEndpoint)
        val blob = AssistClient.sealToRecordPubkey(
            resolvePubkey(to),
            org.json.JSONObject().put("kind", "call").put("ts", System.currentTimeMillis()),
        ) ?: return "cannot seal"
        val signal = if (accept) "accept" else "decline"
        return when (val r = client.signal(to, identity.number.plain, signal, blob)) {
            is AssistClient.Result.Ok -> if (accept) "connected over the assist path" else "declined"
            is AssistClient.Result.Err -> "signal failed: ${r.message}"
        }
    }

    /** Registry lookup for a number → base64 Ed25519 pubkey (or ""). */
    private fun resolvePubkey(number: String): String {
        val client = AssistClient(NetGraph.assistEndpoint)
        val r = client.resolve(number)
        val rec = (r as? AssistClient.Result.Ok)?.json?.optJSONObject("record") ?: return ""
        return rec.optString("pubkey", "")
    }
}
