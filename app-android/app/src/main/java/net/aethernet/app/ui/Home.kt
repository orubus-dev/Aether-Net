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
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
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
import net.aethernet.app.net.RegistryClient
import net.aethernet.app.relay.RelayService

/** Release name, kept in sync with app-android/VERSION. */
object AppVersion {
    const val NAME = "0.1.0"
}

/** Which bottom tab is open. */
enum class Tab { CHATS, NUMBERS, SITES, SETTINGS }

/** Simple app-shell state shared with MainActivity. */
object Shell {
    val tab = MutableStateFlow(Tab.CHATS)
    val openChat = MutableStateFlow<String?>(null) // number of open chat
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
                    Tab.CHATS -> ChatsTab(identity)
                    Tab.NUMBERS -> NumbersTab(identity, handle)
                    Tab.SITES -> SitesTab()
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
        (Tab.CHATS to Icons.Default.Send) to "Chats",
        (Tab.NUMBERS to Icons.Default.Add) to "Numbers",
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
private fun SitesTab() {
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
            Column(Modifier.padding(22.dp)) {
                Text("me.ae — your site on AetherNet", style = GlassTypography.titleMedium)
                Spacer(Modifier.height(8.dp))
                Text(
                    "Import from GitHub or local files, deploy from this app, and your site is served as content-addressed packets cached by relays — reachable at yourname.me.ae from any AetherNet node.",
                    style = GlassTypography.bodySmall,
                )
                Spacer(Modifier.height(10.dp))
                Text(
                    "Landing with Phase 10/11 runtime integration: hosting upload model is already implemented in the core; this screen becomes its live front-end.",
                    style = GlassTypography.bodySmall,
                    color = Glass.InkFaint,
                )
            }
        }
        Spacer(Modifier.height(12.dp))
        GlassCard(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(22.dp)) {
                Text("Game hosting", style = GlassTypography.titleMedium)
                Spacer(Modifier.height(8.dp))
                Text(
                    "Elite and prestige nodes can host small-game relays: state packets ride the same encrypted L4 lanes, so friends on the mesh join by number — no servers, no accounts.",
                    style = GlassTypography.bodySmall,
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
                Text("Number registry claim", style = GlassTypography.titleMedium)
                Spacer(Modifier.height(4.dp))
                Text(
                    "Make your number globally unique on the GitHub registry (orubus-dev/Aether-Net). Create a fine-grained token with contents:write on that repo and paste it here. On collision the app bumps its counter and re-derives — you never manage that by hand.",
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
                    if (busy) "Claiming…" else "Claim on registry",
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
                    NetGraph.setHandle(handle)
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
