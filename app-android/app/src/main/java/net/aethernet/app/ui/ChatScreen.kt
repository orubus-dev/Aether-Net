package net.aethernet.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material.icons.filled.Send
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import net.aethernet.app.Identity
import net.aethernet.app.NetGraph
import net.aethernet.app.data.MessageStore

/**
 * One conversation: direct, end-to-end encrypted over the LAN transport.
 * Delivery states are honest: "queued" means the peer was unreachable so the
 * relay holds the message and delivers on their next beacon (store-and-forward).
 */
@Composable
fun ChatScreen(identity: Identity, peerNumber: String, onBack: () -> Unit) {
    var input by remember { mutableStateOf("") }
    var tick by remember { mutableStateOf(0) }
    var viaAssist by remember { mutableStateOf(false) }
    val listState = rememberLazyListState()

    LaunchedEffect(Unit) {
        NetGraph.transport?.events?.collect { tick++ }
    }

    LiquidBackground(Modifier.fillMaxSize())
    Column(
        Modifier
            .fillMaxSize()
            .systemBarsPadding()
            .imePadding()
    ) {
        // Header
        Row(
            Modifier
                .fillMaxWidth()
                .padding(horizontal = 12.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(
                Modifier
                    .clip(RoundedCornerShape(16.dp))
                    .background(Glass.GlassFill)
                    .padding(6.dp)
            ) {
                Icon(
                    Icons.Default.ArrowBack,
                    contentDescription = "Back",
                    tint = Glass.Ink,
                    modifier = Modifier.padding(4.dp),
                )
            }
            Spacer(Modifier.width(10.dp))
            Box(Modifier.weight(1f)) {
                Column {
                    Text(peerNumber.chunked(3).joinToString(" "), style = GlassTypography.titleMedium)
                    val peer = NetGraph.transport?.peerByNumber(peerNumber)
                    Text(
                        if (peer != null) "on the mesh · ${peer.name}" else "awaiting beacon — messages queue",
                        style = GlassTypography.bodySmall,
                    )
                }
            }
        }

        // Messages
        LazyColumn(
            state = listState,
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            val msgs = NetGraph.messages.messages(peerNumber)
            items(msgs, key = { it.id }) { m ->
                Bubble(m)
            }
        }

        // Composer
        Row(
            Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            FilterChip(
                selected = viaAssist,
                onClick = { viaAssist = !viaAssist },
                label = { Text(if (viaAssist) "assist path" else "mesh path") },
                colors = FilterChipDefaults.filterChipColors(
                    containerColor = Glass.GlassFill,
                    selectedContainerColor = Glass.AccentGlow.copy(alpha = 0.35f),
                    labelColor = Glass.InkDim,
                    selectedLabelColor = Glass.Ink,
                ),
            )
        }
        Row(
            Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OutlinedTextField(
                value = input,
                onValueChange = { input = it },
                placeholder = { Text("Encrypted message…", color = Glass.InkFaint) },
                colors = OutlinedTextFieldDefaults.colors(
                    focusedTextColor = Glass.Ink,
                    unfocusedTextColor = Glass.Ink,
                    focusedBorderColor = Glass.AccentGlow,
                    unfocusedBorderColor = Glass.GlassBorder,
                    cursorColor = Glass.AccentGlow,
                ),
                shape = RoundedCornerShape(20.dp),
                modifier = Modifier.weight(1f),
            )
            Spacer(Modifier.width(8.dp))
            Box(
                Modifier
                    .widthIn(min = 52.dp)
                    .height(52.dp)
                    .clip(RoundedCornerShape(18.dp))
                    .background(Glass.AccentGlow.copy(alpha = 0.9f))
                    .clickable {
                        val text = input.trim()
                        if (text.isNotEmpty()) {
                            if (viaAssist) {
                                // Internet-assisted delivery: sealed envelope
                                // through the endpoint, which sees only ciphertext.
                                val id = "a" + System.currentTimeMillis().toString(16)
                                var ok = false
                                var err = ""
                                runCatching {
                                    val client = net.aethernet.app.net.AssistClient(NetGraph.assistEndpoint)
                                    val resolve = client.resolve(peerNumber)
                                    val rec = (resolve as? net.aethernet.app.net.AssistClient.Result.Ok)
                                        ?.json?.optJSONObject("record")
                                    if (rec == null) {
                                        err = "peer not on registry"
                                    } else {
                                        val blob = net.aethernet.app.net.AssistClient.sealToRecordPubkey(
                                            rec.optString("pubkey"),
                                            org.json.JSONObject()
                                                .put("kind", "message")
                                                .put("text", text)
                                                .put("ts", System.currentTimeMillis()),
                                        )
                                        val drop = blob?.let {
                                            client.mailDrop(peerNumber, identity.number.plain, it, "message", 3 * 24 * 3600)
                                        }
                                        ok = drop is net.aethernet.app.net.AssistClient.Result.Ok
                                        if (!ok) err = (drop as? net.aethernet.app.net.AssistClient.Result.Err)?.message ?: "drop failed"
                                    }
                                }.onFailure { err = it.message ?: "assist error" }
                                NetGraph.messages.append(
                                    peerNumber,
                                    MessageStore.Msg(
                                        id = id,
                                        ts = System.currentTimeMillis(),
                                        outgoing = true,
                                        text = text,
                                        status = if (ok) MessageStore.Status.SENT else MessageStore.Status.PENDING,
                                    )
                                )
                            } else {
                            val t = NetGraph.transport
                            val res = t?.sendText(peerNumber, text)
                            val id = (res as? net.aethernet.app.net.LanTransport.SendResult.Sent)?.msgId
                                ?: "q" + System.currentTimeMillis().toString(16)
                            NetGraph.messages.append(
                                peerNumber,
                                MessageStore.Msg(
                                    id = id,
                                    ts = System.currentTimeMillis(),
                                    outgoing = true,
                                    text = text,
                                    status = if (res is net.aethernet.app.net.LanTransport.SendResult.Sent)
                                        MessageStore.Status.SENT
                                    else MessageStore.Status.PENDING,
                                )
                            )
                            }
                            input = ""
                            tick++
                        }
                    },
                contentAlignment = Alignment.Center,
            ) {
                Icon(Icons.Default.Send, contentDescription = "Send", tint = Glass.FieldBottom)
            }
        }
        Spacer(Modifier.height(4.dp))
    }

    LaunchedEffect(tick) {
        val msgs = NetGraph.messages.messages(peerNumber)
        if (msgs.isNotEmpty()) listState.animateScrollToItem(msgs.size - 1)
    }
}

@Composable
private fun Bubble(m: MessageStore.Msg) {
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = if (m.outgoing) Arrangement.End else Arrangement.Start,
    ) {
        Column(
            Modifier
                .widthIn(max = 300.dp)
                .clip(
                    RoundedCornerShape(
                        topStart = if (m.outgoing) 18.dp else 4.dp,
                        topEnd = if (m.outgoing) 4.dp else 18.dp,
                        bottomStart = 18.dp,
                        bottomEnd = 18.dp,
                    )
                )
                .background(if (m.outgoing) Glass.BubbleOut else Glass.BubbleIn)
                .padding(horizontal = 14.dp, vertical = 10.dp)
        ) {
            Text(m.text, color = Glass.Ink, fontSize = 15.sp, lineHeight = 21.sp)
            Spacer(Modifier.height(2.dp))
            Text(
                when {
                    !m.outgoing -> timeLabel(m.ts)
                    m.status == MessageStore.Status.PENDING -> "queued · will deliver on next sighting"
                    else -> timeLabel(m.ts) + " · " +
                        if (m.status == MessageStore.Status.DELIVERED) "delivered" else "sent"
                },
                color = if (m.outgoing && m.status == MessageStore.Status.DELIVERED) Glass.Ok else Glass.InkFaint,
                fontSize = 10.sp,
            )
        }
    }
}

private fun timeLabel(ts: Long): String {
    val cal = java.util.Calendar.getInstance().apply { timeInMillis = ts }
    return "%02d:%02d".format(cal.get(java.util.Calendar.HOUR_OF_DAY), cal.get(java.util.Calendar.MINUTE))
}
