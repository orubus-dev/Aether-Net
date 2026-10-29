package net.aethernet.app.ui

import android.content.Context
import android.widget.Toast
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Call
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Send
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.aethernet.app.AetherNumber
import net.aethernet.app.Identity
import net.aethernet.app.NetGraph
import net.aethernet.app.data.MessageStore
import net.aethernet.app.net.AssistClient
import net.aethernet.app.net.AssistPoller
import net.aethernet.app.net.RegistryClient
import net.aethernet.app.relay.RelayService

/** Release name, kept in sync with app-android/VERSION. */
object AppVersion {
    const val NAME = "0.2.0"
}

/** Which bottom tab is open. */
enum class Tab { PHONE, CHATS, SITES, SETTINGS }

/** Simple app-shell state shared with MainActivity. */
object Shell {
    val tab = MutableStateFlow(Tab.PHONE)
    val openChat = MutableStateFlow<String?>(null) // number of open chat
}

/**
 * Live assist-layer events shared between the poller and the UI: incoming
 * calls ring here, incoming assist mail lands in the message store.
 */
object AssistBus {
    sealed class CallEvent { data class Ring(val from: String) : CallEvent(); object Idle : CallEvent() }
    val call = MutableStateFlow<CallEvent>(CallEvent.Idle)
    val tick = MutableStateFlow(0)
}

@Composable
fun Home(identity: Identity, handle: String?) {
    val tab by Shell.tab.collectAsState()
    val openChat by Shell.openChat.collectAsState()
    val context = LocalContext.current
    val relayOn = remember { MutableStateFlow(false) }
    val relayState by relayOn.collectAsState()

    LaunchedEffect(Unit) {
        NetGraph.transport?.running?.collect { relayOn.value = it }
    }

    // Assist poller: rides the internet-assisted path when the mesh is out
    // of reach. Encrypted envelopes only — the endpoint cannot read them.
    LaunchedEffect(Unit) {
        while (true) {
            kotlinx.coroutines.delay(8_000)
            val r = withContext(Dispatchers.IO) {
                runCatching { AssistPoller.poll(identity) }.getOrNull()
            }
            if (r == true) AssistBus.tick.value++
        }
    }

    LiquidBackground(Modifier.fillMaxSize())

    when (val chat = openChat) {
        null -> Column(
            Modifier
                .fillMaxSize()
                .systemBarsPadding()
                .imePadding()
        ) {
            Box(Modifier.weight(1f)) {
                when (tab) {
                    Tab.PHONE -> DialerTab(identity)
                    Tab.CHATS -> ChatsTab(identity)
                    Tab.SITES -> SitesTab(identity)
                    Tab.SETTINGS -> SettingsTab(identity, handle)
                }
            }
            GlassBar(
                current = tab,
                onTab = { Shell.tab.value = it },
            )
        }
        else -> ChatScreen(identity, chat) { Shell.openChat.value = null }
    }
}

@Composable
private fun GlassBar(current: Tab, onTab: (Tab) -> Unit) {
    val items = listOf(
        (Tab.PHONE to Icons.Default.Call) to "Phone",
        (Tab.CHATS to Icons.Default.Send) to "Chats",
        (Tab.SITES to Icons.Default.Home) to "Sites",
        (Tab.SETTINGS to Icons.Default.Settings) to "Settings",
    )
    Box(
        Modifier
            .padding(horizontal = 16.dp, vertical = 10.dp)
            .fillMaxWidth()
            .clip(RoundedCornerShape(28.dp))
            .background(Glass.GlassFill)
    ) {
        Row(
            Modifier
                .fillMaxWidth()
                .padding(vertical = 10.dp),
            horizontalArrangement = Arrangement.SpaceEvenly,
        ) {
            items.forEach { (pair, label) ->
                val (t, icon) = pair
                val selected = current == t
                Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    modifier = Modifier
                        .clip(RoundedCornerShape(18.dp))
                        .clickable { onTab(t) }
                        .padding(horizontal = 14.dp, vertical = 4.dp),
                ) {
                    Icon(
                        icon,
                        contentDescription = label,
                        tint = if (selected) Glass.Accent else Glass.InkFaint,
                    )
                    Text(
                        label,
                        fontSize = 10.sp,
                        color = if (selected) Glass.Accent else Glass.InkFaint,
                        fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
                    )
                }
            }
        }
    }
}

/* ------------------------------- PHONE TAB ------------------------------- */

/** Shared dial state so the pad and the call card agree. */
object Dialer {
    val dialed = MutableStateFlow("")
    val status = MutableStateFlow("")
}

@Composable
private fun DialerTab(identity: Identity) {
    val scope = rememberCoroutineScope()
    val dialed by Dialer.dialed.collectAsState()
    val status by Dialer.status.collectAsState()
    val call by AssistBus.call.collectAsState()
    val context = LocalContext.current

    Column(
        Modifier
            .fillMaxSize()
            .padding(horizontal = 24.dp)
            .verticalScroll(rememberScrollState())
    ) {
        Spacer(Modifier.height(18.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text("Phone", style = GlassTypography.headlineMedium)
                Text("Calls ride the mesh; the assist path rings when the mesh can’t.", style = GlassTypography.bodySmall)
            }
            StatusDot(
                on = NetGraph.transport?.running?.value == true,
                label = if (NetGraph.transport?.running?.value == true) "mesh live" else "assist path",
            )
        }
        Spacer(Modifier.height(26.dp))

        Text(
            if (dialed.isEmpty()) " " else dialed.chunked(3).joinToString(" "),
            fontSize = 34.sp,
            fontWeight = FontWeight.Bold,
            color = Glass.Ink,
            modifier = Modifier.fillMaxWidth(),
            textAlign = androidx.compose.ui.text.style.TextAlign.Center,
        )
        Spacer(Modifier.height(18.dp))

        val rows = listOf(
            listOf("1", "2", "3"),
            listOf("4", "5", "6"),
            listOf("7", "8", "9"),
            listOf("⌫", "0", "☎"),
        )
        rows.forEach { row ->
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceEvenly,
            ) {
                row.forEach { key ->
                    Box(
                        Modifier
                            .size(72.dp)
                            .clip(RoundedCornerShape(36.dp))
                            .background(if (key == "☎") Glass.AccentGlow.copy(alpha = 0.9f) else Glass.GlassFill)
                            .clickable {
                                when (key) {
                                    "⌫" -> Dialer.dialed.value = dialed.dropLast(1)
                                    "☎" -> {
                                        val n = dialed
                                        if (n.length == 10) {
                                            Dialer.status.value = "resolving…"
                                            scope.launch {
                                                Dialer.status.value = withContext(Dispatchers.IO) {
                                                    AssistPoller.ring(identity, n)
                                                }
                                            }
                                        } else {
                                            Dialer.status.value = "enter a full ten-digit number"
                                        }
                                    }
                                    else -> if (dialed.length < 10) Dialer.dialed.value = dialed + key
                                }
                            },
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(key, fontSize = 24.sp, color = if (key == "☎") Glass.FieldBottom else Glass.Ink, fontWeight = FontWeight.SemiBold)
                    }
                }
            }
            Spacer(Modifier.height(12.dp))
        }

        Text(status, fontSize = 14.sp, color = Glass.AccentGlow, modifier = Modifier.padding(top = 6.dp))

        when (val c = call) {
            is AssistBus.CallEvent.Ring -> GlassCard(Modifier.fillMaxWidth().padding(top = 14.dp)) {
                Column(Modifier.padding(20.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text("INCOMING CALL", color = Glass.InkDim, fontSize = 12.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 2.sp)
                    Spacer(Modifier.height(8.dp))
                    NumberDisplay(c.from, fontSize = 28.sp)
                    Spacer(Modifier.height(14.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        GlowButton("Accept", height = 48.dp) {
                            AssistBus.call.value = AssistBus.CallEvent.Idle
                            scope.launch {
                                Dialer.status.value = withContext(Dispatchers.IO) {
                                    AssistPoller.answer(identity, c.from, accept = true)
                                }
                            }
                        }
                        GlowButton("Decline", height = 48.dp) {
                            val from = c.from
                            AssistBus.call.value = AssistBus.CallEvent.Idle
                            scope.launch {
                                withContext(Dispatchers.IO) { AssistPoller.answer(identity, from, accept = false) }
                            }
                        }
                    }
                }
            }
            AssistBus.CallEvent.Idle -> {}
        }
        Spacer(Modifier.height(14.dp))
    }
}

/* ------------------------------- CHATS TAB ------------------------------- */

@Composable
private fun ChatsTab(identity: Identity) {
    val context = LocalContext.current
    var tick by remember { mutableStateOf(0) }
    var addNumber by remember { mutableStateOf("") }

    LaunchedEffect(Unit) {
        NetGraph.transport?.events?.collect {
            tick++ // any event refreshes the conversation list
        }
    }

    Column(
        Modifier
            .fillMaxSize()
            .padding(horizontal = 20.dp)
            .verticalScroll(rememberScrollState())
    ) {
        Spacer(Modifier.height(18.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text("Chats", style = GlassTypography.headlineMedium)
                Text("Direct over the mesh. End-to-end encrypted.", style = GlassTypography.bodySmall)
            }
            StatusDot(
                on = NetGraph.transport?.running?.value == true,
                label = if (NetGraph.transport?.running?.value == true) "relay live" else "relay off",
            )
        }
        Spacer(Modifier.height(14.dp))

        GlassCard(Modifier.fillMaxWidth()) {
            Row(
                Modifier.padding(12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                OutlinedTextField(
                    value = addNumber,
                    onValueChange = { addNumber = it },
                    placeholder = { Text("700 0000 000", color = Glass.InkFaint) },
                    singleLine = true,
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedTextColor = Glass.Ink,
                        unfocusedTextColor = Glass.Ink,
                        focusedBorderColor = Glass.AccentGlow,
                        unfocusedBorderColor = Glass.GlassBorder,
                        cursorColor = Glass.AccentGlow,
                    ),
                    modifier = Modifier.weight(1f),
                )
                Spacer(Modifier.width(8.dp))
                GlowButton("Start", enabled = AetherNumber.parse(addNumber) != null, height = 48.dp) {
                    val n = AetherNumber.parse(addNumber)!!.plain
                    NetGraph.messages.ensureConversation(n)
                    addNumber = ""
                    Shell.openChat.value = n
                }
            }
        }
        Spacer(Modifier.height(12.dp))

        val conversations = remember(tick) { NetGraph.messages.conversations() }
        if (conversations.isEmpty()) {
            GlassCard(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(20.dp)) {
                    Text("No conversations yet", style = GlassTypography.titleMedium)
                    Spacer(Modifier.height(4.dp))
                    Text(
                        "Start one by number above, or wait — every AetherNet device that beacons this phone appears here.",
                        style = GlassTypography.bodySmall,
                    )
                }
            }
        }
        conversations.forEach { (number, last) ->
            GlassCard(
                Modifier
                    .fillMaxWidth()
                    .padding(bottom = 10.dp)
                    .clickable { Shell.openChat.value = number },
                corner = 20.dp,
            ) {
                Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                    Box(
                        Modifier
                            .weight(1f)
                    ) {
                        Column {
                            Text(number.chunked(3).joinToString(" "), style = GlassTypography.titleMedium)
                            Text(
                                last?.text ?: "say hello",
                                style = GlassTypography.bodySmall,
                                maxLines = 1,
                            )
                        }
                    }
                    last?.let {
                        Text(
                            when (it.status) {
                                MessageStore.Status.PENDING -> "queued"
                                MessageStore.Status.SENT -> "sent"
                                MessageStore.Status.DELIVERED -> "delivered"
                            },
                            color = if (it.status == MessageStore.Status.DELIVERED) Glass.Ok else Glass.InkFaint,
                            fontSize = 11.sp,
                        )
                    }
                }
            }
        }
        Spacer(Modifier.height(12.dp))
    }
}

/* ------------------------------ NUMBERS TAB ------------------------------ */

@Composable
private fun NumbersTab(identity: Identity, handle: String?) {
    val clipboard = LocalClipboardManager.current
    val context = LocalContext.current
    val pubB64 = remember {
        android.util.Base64.encodeToString(identity.publicKey, android.util.Base64.NO_WRAP)
    }

    Column(
        Modifier
            .fillMaxSize()
            .padding(horizontal = 20.dp)
            .verticalScroll(rememberScrollState())
    ) {
        Spacer(Modifier.height(18.dp))
        Text("My number", style = GlassTypography.headlineMedium)
        Text("Yours alone — derived from your key, proven by the registry.", style = GlassTypography.bodySmall)
        Spacer(Modifier.height(16.dp))

        GlassCard(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(22.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                NumberDisplay(identity.number)
                Spacer(Modifier.height(10.dp))
                TierBadge(identity.tier)
                if (handle != null) {
                    Spacer(Modifier.height(12.dp))
                    Text(
                        "$handle@" + if (identity.tier == net.aethernet.app.Tier.STANDARD) "me.ae" else "ae",
                        style = GlassTypography.titleMedium,
                        color = Glass.Accent,
                    )
                }
                Spacer(Modifier.height(16.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    GlowButton(
                        "Copy",
                        height = 44.dp,
                    ) {
                        clipboard.setText(AnnotatedString(identity.number.plain))
                        Toast.makeText(context, "Number copied", Toast.LENGTH_SHORT).show()
                    }
                    GlowButton("Key: ${identity.fingerprint.take(6)}…", enabled = false, height = 44.dp) {}
                }
            }
        }

        Spacer(Modifier.height(14.dp))
        GlassCard(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(20.dp)) {
                Text("How your number is made", style = GlassTypography.titleMedium)
                Spacer(Modifier.height(6.dp))
                Text(
                    "Not drawn from a pool. BLAKE2b-256(pubkey || counter) decides the seven free digits behind your tier prefix, so the number is bound to the device key that owns it. The registry CI recomputes every published number from its key — a number that exists provably derives from the key that claims it.",
                    style = GlassTypography.bodySmall,
                )
                Spacer(Modifier.height(10.dp))
                Text(
                    "Public key (base64):",
                    style = GlassTypography.bodySmall,
                    fontWeight = FontWeight.SemiBold,
                )
                Text(pubB64, style = GlassTypography.bodySmall, color = Glass.InkFaint)
            }
        }
        Spacer(Modifier.height(14.dp))
    }
}

/* ------------------------------- SITES TAB ------------------------------- */

@Composable
private fun SitesTab(identity: Identity) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var url by remember { mutableStateOf("") }
    var browsing by remember { mutableStateOf(false) }
    var html by remember { mutableStateOf<String?>(null) }
    var status by remember { mutableStateOf("") }

    fun open(target: String) {
        status = "resolving…"
        scope.launch {
            val result = withContext(Dispatchers.IO) {
                runCatching {
                    val client = AssistClient(NetGraph.assistEndpoint)
                    val r = client.sitesGet(target, "/")
                    when (r) {
                        is AssistClient.Result.Ok -> {
                            val b64 = r.json.optString("content")
                            val bytes = android.util.Base64.decode(b64, android.util.Base64.NO_WRAP)
                            String(bytes, Charsets.UTF_8)
                        }
                        is AssistClient.Result.Err -> null
                    }
                }.getOrNull()
            }
            if (result != null) {
                html = result
                browsing = true
                status = ""
            } else {
                status = "cannot open $target — is the site published on the registry?"
            }
        }
    }

    Column(
        Modifier
            .fillMaxSize()
            .padding(horizontal = 20.dp)
            .verticalScroll(rememberScrollState())
    ) {
        Spacer(Modifier.height(18.dp))
        Text("Sites", style = GlassTypography.headlineMedium)
        Text("The web that lives on the mesh.", style = GlassTypography.bodySmall)
        Spacer(Modifier.height(16.dp))

        GlassCard(Modifier.fillMaxWidth()) {
            Row(Modifier.padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
                OutlinedTextField(
                    value = url,
                    onValueChange = { url = it },
                    placeholder = { Text("genesis.me.ae", color = Glass.InkFaint) },
                    singleLine = true,
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedTextColor = Glass.Ink,
                        unfocusedTextColor = Glass.Ink,
                        focusedBorderColor = Glass.AccentGlow,
                        unfocusedBorderColor = Glass.GlassBorder,
                        cursorColor = Glass.AccentGlow,
                    ),
                    modifier = Modifier.weight(1f),
                )
                Spacer(Modifier.width(8.dp))
                GlowButton("Open", height = 48.dp, enabled = url.isNotBlank()) {
                    open(url.trim().lowercase())
                }
            }
        }
        Text(status, fontSize = 13.sp, color = Glass.AccentGlow, modifier = Modifier.padding(horizontal = 4.dp))

        if (browsing && html != null) {
            GlassCard(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(10.dp)) {
                    Text(url, style = GlassTypography.titleMedium)
                    Spacer(Modifier.height(8.dp))
                    androidx.compose.ui.viewinterop.AndroidView(
                        factory = { ctx ->
                            android.webkit.WebView(ctx).apply {
                                settings.javaScriptEnabled = false
                                settings.loadWithOverviewMode = true
                            }
                        },
                        update = { wv -> wv.loadDataWithBaseURL(null, html ?: "", "text/html", "utf-8", null) },
                        modifier = Modifier.fillMaxWidth().height(420.dp),
                    )
                }
            }
        }

        GlassCard(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(22.dp)) {
                Text("Your site on the network", style = GlassTypography.titleMedium)
                Spacer(Modifier.height(8.dp))
                Text(
                    "Import files, deploy, and your site is served as content-addressed packages cached by relays — reachable at yourname${if (identity.tier == net.aethernet.app.Tier.STANDARD) ".me.ae" else ".ae"} from any AetherNet node or any browser through the assist endpoint.",
                    style = GlassTypography.bodySmall,
                )
                Spacer(Modifier.height(10.dp))
                Text(
                    "Deploying from the phone lands with the native site importer (M2). The web client at the assist endpoint can publish today.",
                    style = GlassTypography.bodySmall,
                    color = Glass.InkFaint,
                )
            }
        }
        Spacer(Modifier.height(14.dp))
    }
}

/* ----------------------------- SETTINGS TAB ------------------------------ */

@Composable
private fun SettingsTab(identity: Identity, handle: String?) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var token by remember {
        mutableStateOf(
            context.getSharedPreferences("aethernet", Context.MODE_PRIVATE)
                .getString("gh_token", "") ?: ""
        )
    }
    var handleInput by remember { mutableStateOf(handle ?: "") }
    var claimStatus by remember { mutableStateOf("not claimed yet") }
    var busy by remember { mutableStateOf(false) }
    var nameInput by remember { mutableStateOf(NetGraph.displayName ?: "") }
    var emailInput by remember { mutableStateOf(NetGraph.emailPointer ?: "") }
    var assistUrl by remember { mutableStateOf(NetGraph.assistEndpoint) }
    var assistStatus by remember { mutableStateOf("") }

    Column(
        Modifier
            .fillMaxSize()
            .padding(horizontal = 20.dp)
            .verticalScroll(rememberScrollState())
    ) {
        Spacer(Modifier.height(18.dp))
        Text("Settings", style = GlassTypography.headlineMedium)
        Text("Identity, registry claim, relay.", style = GlassTypography.bodySmall)
        Spacer(Modifier.height(16.dp))

        GlassCard(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(20.dp)) {
                Text("Your identity", style = GlassTypography.titleMedium)
                Spacer(Modifier.height(10.dp))
                OutlinedTextField(
                    value = nameInput,
                    onValueChange = { nameInput = it },
                    placeholder = { Text("display name", color = Glass.InkFaint) },
                    singleLine = true,
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedTextColor = Glass.Ink,
                        unfocusedTextColor = Glass.Ink,
                        focusedBorderColor = Glass.AccentGlow,
                        unfocusedBorderColor = Glass.GlassBorder,
                        cursorColor = Glass.AccentGlow,
                    ),
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = emailInput,
                    onValueChange = { emailInput = it },
                    placeholder = { Text("email pointer (reachable off-net)", color = Glass.InkFaint) },
                    singleLine = true,
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedTextColor = Glass.Ink,
                        unfocusedTextColor = Glass.Ink,
                        focusedBorderColor = Glass.AccentGlow,
                        unfocusedBorderColor = Glass.GlassBorder,
                        cursorColor = Glass.AccentGlow,
                    ),
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height(8.dp))
                GlowButton("Save profile", modifier = Modifier.fillMaxWidth()) {
                    NetGraph.setProfile(nameInput.trim(), emailInput.trim())
                    assistStatus = "profile saved — included in your next claim"
                }
                Spacer(Modifier.height(10.dp))
                Text(
                    "fingerprint ${identity.fingerprint.chunked(4).joinToString(" ")}",
                    style = GlassTypography.bodySmall,
                    color = Glass.InkFaint,
                )
            }
        }

        Spacer(Modifier.height(12.dp))
        GlassCard(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(20.dp)) {
                Text("Assist endpoint", style = GlassTypography.titleMedium)
                Spacer(Modifier.height(4.dp))
                Text(
                    "Stateless ciphertext pipe over GitHub storage — no backend holds keys. Runs at aether-net.vercel.app, or on your own box: node api/sandbox-server.js",
                    style = GlassTypography.bodySmall,
                )
                Spacer(Modifier.height(12.dp))
                OutlinedTextField(
                    value = assistUrl,
                    onValueChange = { assistUrl = it },
                    placeholder = { Text("https://aether-net.vercel.app", color = Glass.InkFaint) },
                    singleLine = true,
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedTextColor = Glass.Ink,
                        unfocusedTextColor = Glass.Ink,
                        focusedBorderColor = Glass.AccentGlow,
                        unfocusedBorderColor = Glass.GlassBorder,
                        cursorColor = Glass.AccentGlow,
                    ),
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height(8.dp))
                GlowButton("Save & test", modifier = Modifier.fillMaxWidth()) {
                    NetGraph.setAssistEndpoint(assistUrl, context)
                    assistStatus = "testing…"
                    scope.launch {
                        assistStatus = withContext(Dispatchers.IO) {
                            when (val r = AssistClient(NetGraph.assistEndpoint).health()) {
                                is AssistClient.Result.Ok ->
                                    "healthy — writes ${if (r.json.optJSONObject("capabilities")?.optBoolean("writes") == true) "enabled" else "read-only (endpoint token missing)"}"
                                is AssistClient.Result.Err -> "unreachable: ${r.message}"
                            }
                        }
                    }
                }
                Text(assistStatus, fontSize = 13.sp, color = Glass.AccentGlow, modifier = Modifier.padding(top = 8.dp))
            }
        }

        GlassCard(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(20.dp)) {
                Text("Number registry claim", style = GlassTypography.titleMedium)
                Spacer(Modifier.height(4.dp))
                Text(
                    "Make your number globally unique on the GitHub registry (orubus-dev/Aether-Net). Two ways: claim through the assist endpoint with no token, or paste a fine-grained token with contents:write and claim directly. On collision the app bumps its counter and re-derives — you never manage that by hand.",
                    style = GlassTypography.bodySmall,
                )
                Spacer(Modifier.height(12.dp))
                OutlinedTextField(
                    value = token,
                    onValueChange = {
                        token = it
                        context.getSharedPreferences("aethernet", Context.MODE_PRIVATE)
                            .edit().putString("gh_token", it).apply()
                    },
                    placeholder = { Text("github_pat_…", color = Glass.InkFaint) },
                    singleLine = true,
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedTextColor = Glass.Ink,
                        unfocusedTextColor = Glass.Ink,
                        focusedBorderColor = Glass.AccentGlow,
                        unfocusedBorderColor = Glass.GlassBorder,
                        cursorColor = Glass.AccentGlow,
                    ),
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = handleInput,
                    onValueChange = { handleInput = it },
                    placeholder = { Text("handle (genesis → genesis.me.ae)", color = Glass.InkFaint) },
                    singleLine = true,
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedTextColor = Glass.Ink,
                        unfocusedTextColor = Glass.Ink,
                        focusedBorderColor = Glass.AccentGlow,
                        unfocusedBorderColor = Glass.GlassBorder,
                        cursorColor = Glass.AccentGlow,
                    ),
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height(10.dp))
                GlowButton(
                    if (busy) "Claiming…" else "Claim via assist (no token)",
                    enabled = !busy,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    busy = true
                    claimStatus = "claiming…"
                    scope.launch {
                        claimStatus = withContext(Dispatchers.IO) {
                            val pubB64 = android.util.Base64.encodeToString(
                                identity.publicKey, android.util.Base64.NO_WRAP
                            )
                            AssistClient(NetGraph.assistEndpoint).claimFlow(
                                identityPubKeyB64 = pubB64,
                                tierName = identity.tier.name,
                                deriveNumber = { c -> AetherNumber.derive(identity.publicKey, c, identity.tier) },
                                startCounter = identity.counter,
                                handle = handleInput.trim().lowercase().ifEmpty { null },
                                displayName = NetGraph.displayName,
                                emailPointer = NetGraph.emailPointer,
                                onRederive = { NetGraph.rederiveWithCounter(it) },
                            )
                        }
                        busy = false
                    }
                }
                Spacer(Modifier.height(8.dp))
                GlowButton(
                    if (busy) "Claiming…" else "Claim with GitHub token",
                    enabled = !busy && token.isNotBlank(),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    busy = true
                    claimStatus = "claiming…"
                    scope.launch {
                        claimStatus = withContext(Dispatchers.IO) {
                            claimFlow(identity, handleInput.trim().lowercase(), token)
                        }
                        busy = false
                    }
                }
                Spacer(Modifier.height(8.dp))
                Text(claimStatus, style = GlassTypography.bodySmall, color = Glass.InkDim)
            }
        }

        Spacer(Modifier.height(12.dp))
        GlassCard(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(20.dp)) {
                Text("Relay", style = GlassTypography.titleMedium)
                Spacer(Modifier.height(4.dp))
                Text(
                    "The relay keeps this phone acting as a node: it beacons, routes, and delivers parked messages even when the app is closed.",
                    style = GlassTypography.bodySmall,
                )
                Spacer(Modifier.height(10.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    GlowButton("Start relay", height = 46.dp) {
                        RelayService.start(context)
                    }
                    GlowButton("Stop", height = 46.dp) {
                        RelayService.stop(context)
                    }
                }
            }
        }

        Spacer(Modifier.height(12.dp))
        GlassCard(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(20.dp)) {
                Text("About", style = GlassTypography.titleMedium)
                Spacer(Modifier.height(6.dp))
                Text(
                    "AetherNet app v" + AppVersion.NAME + " — the client of the AetherNet carrier network: L0 physical underlay to L5 services in the Aether-Net core, talk-layer v0 on this device. Encrypted by default. Identity without exposure.",
                    style = GlassTypography.bodySmall,
                )
            }
        }
        Spacer(Modifier.height(16.dp))
    }
}

/**
 * The full claim flow: number claim first, auto-retrying on collision by
 * re-deriving at the next counter, then the handle claim bound to it.
 */
private fun claimFlow(identity: Identity, handle: String?, token: String): String {
    if (token.isBlank()) return "add a GitHub token first"
    val client = RegistryClient(token)
    val pubB64 = android.util.Base64.encodeToString(
        identity.publicKey, android.util.Base64.NO_WRAP
    )
    var attempt = identity.counter
    var lastMessage = ""

    // Up to 5 counter bumps; each bump re-derives from the same key.
    while (attempt < identity.counter + 5) {
        val number = AetherNumber.derive(identity.publicKey, attempt, identity.tier)
        when (val result = client.claimNumber(
            number = number.plain,
            tierName = identity.tier.name,
            pubKeyB64 = pubB64,
            counter = attempt,
            handle = handle,
        )) {
            is RegistryClient.ClaimResult.Claimed -> {
                if (attempt != identity.counter) NetGraph.rederiveWithCounter(attempt)
                if (!handle.isNullOrEmpty()) {
                    val h = client.claimHandle(handle, number.plain)
                    NetGraph.updateHandle(handle)
                    return when (h) {
                        is RegistryClient.ClaimResult.Claimed ->
                            "${number.display} + $handle@${identity.tier.domainSuffix} claimed"
                        is RegistryClient.ClaimResult.AlreadyTaken ->
                            "${number.display} claimed; handle already taken"
                        is RegistryClient.ClaimResult.Error ->
                            "${number.display} claimed; handle error: ${h.message}"
                    }
                }
                return "${number.display} claimed on the registry"
            }
            is RegistryClient.ClaimResult.AlreadyTaken -> {
                lastMessage = "${number.display} already taken — re-deriving"
                attempt++
            }
            is RegistryClient.ClaimResult.Error ->
                return "claim failed: ${result.message}"
        }
    }
    return if (lastMessage.isEmpty()) "claim failed" else lastMessage
}
