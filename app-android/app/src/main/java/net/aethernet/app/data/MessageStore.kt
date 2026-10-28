package net.aethernet.app.data

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.concurrent.ConcurrentHashMap

/**
 * Conversation store v0: one JSON file, loaded into memory, written
 * asynchronously on every change. Good enough for v0.1.0's LAN scale; the
 * native L4/log store takes over when Phase 11 lands in the runtime.
 */
class MessageStore(context: Context) {

    enum class Status { SENT, DELIVERED, PENDING }

    data class Msg(
        val id: String,
        val ts: Long,
        val outgoing: Boolean,
        val text: String,
        var status: Status = Status.SENT,
    )

    private val file = File(context.filesDir, "conversations.json")
    private val byNumber = ConcurrentHashMap<String, MutableList<Msg>>()

    init {
        runCatching { load() }
    }

    private fun load() {
        if (!file.exists()) return
        val root = JSONObject(file.readText())
        val keys = root.keys()
        while (keys.hasNext()) {
            val num = keys.next()
            val arr = root.getJSONArray(num)
            val list = (0 until arr.length()).map { i ->
                val m = arr.getJSONObject(i)
                Msg(
                    id = m.getString("id"),
                    ts = m.getLong("ts"),
                    outgoing = m.getBoolean("outgoing"),
                    text = m.getString("text"),
                    status = Status.valueOf(m.optString("status", "SENT")),
                )
            }.toMutableList()
            byNumber[num] = list
        }
    }

    private fun persist() {
        // Persist on a background thread; the store is tiny at v0 scale.
        Thread {
            runCatching {
                val root = JSONObject()
                for ((k, v) in byNumber) {
                    val arr = JSONArray()
                    for (m in v) {
                        arr.put(
                            JSONObject()
                                .put("id", m.id)
                                .put("ts", m.ts)
                                .put("outgoing", m.outgoing)
                                .put("text", m.text)
                                .put("status", m.status.name)
                        )
                    }
                    root.put(k, arr)
                }
                file.writeText(root.toString())
            }
        }.start()
    }

    fun conversations(): List<Pair<String, Msg?>> =
        byNumber.entries
            .map { (k, v) -> k to v.lastOrNull() }
            .sortedByDescending { (_, last) -> last?.ts ?: 0L }

    fun messages(number: String): List<Msg> = byNumber[number]?.sortedBy { it.ts } ?: emptyList()

    fun append(number: String, msg: Msg) {
        byNumber.getOrPut(number) { mutableListOf() }.add(msg)
        persist()
    }

    fun markDelivered(number: String, msgId: String) {
        val m = byNumber[number]?.firstOrNull { it.id == msgId } ?: return
        m.status = Status.DELIVERED
        persist()
    }

    /** Messages queued for a peer that was offline when sent. */
    fun pendingFor(number: String): List<Msg> =
        (byNumber[number] ?: emptyList()).filter { it.outgoing && it.status == Status.PENDING }

    fun ensureConversation(number: String) {
        if (!byNumber.containsKey(number)) {
            byNumber[number] = mutableListOf()
            persist()
        }
    }
}
