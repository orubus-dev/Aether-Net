package net.aethernet.app.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import net.aethernet.app.Identity
import net.aethernet.app.Tier

/**
 * Onboarding: one screen, one decision (tier), one tap. The identity is
 * generated on-device; the number appears instantly and works offline — the
 * registry claim happens later, whenever connectivity and a GitHub token
 * exist.
 */
@Composable
fun Onboarding(
    onCreate: (Tier) -> Identity,
    onDone: (Identity) -> Unit,
) {
    var tier by remember { mutableStateOf(Tier.STANDARD) }
    var created by remember { mutableStateOf<Identity?>(null) }

    LiquidBackground(Modifier.fillMaxSize())
    Column(
        Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(24.dp),
        verticalArrangement = Arrangement.SpaceBetween,
    ) {
        Column {
            Spacer(Modifier.height(40.dp))
            Text("AetherNet", style = GlassTypography.headlineLarge, color = Glass.Accent)
            Spacer(Modifier.height(8.dp))
            Text(
                "The privacy carrier. No towers, no data centers — your phone is a node, and the network is the people around you.",
                style = GlassTypography.bodyLarge,
            )
        }

        created?.let { id ->
            GlassCard(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(22.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text("THIS IS YOUR NUMBER", color = Glass.InkDim, fontSize = 12.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 2.sp)
                    Spacer(Modifier.height(14.dp))
                    NumberDisplay(id.number, fontSize = 34.sp)
                    Spacer(Modifier.height(10.dp))
                    TierBadge(id.tier)
                    Spacer(Modifier.height(14.dp))
                    Text(
                        "Derived from your device key — it was never picked from a pool, so nobody else can hold it. Works offline from this second.",
                        style = GlassTypography.bodySmall,
                    )
                }
            }
        } ?: GlassCard(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(22.dp)) {
                Text("Choose your tier", style = GlassTypography.titleMedium)
                Spacer(Modifier.height(4.dp))
                Text(
                    "The prefix is your class of service on the network.",
                    style = GlassTypography.bodySmall,
                )
                Spacer(Modifier.height(12.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Tier.entries.forEach { t ->
                        FilterChip(
                            selected = tier == t,
                            onClick = { tier = t },
                            label = { Text("${t.prefix} ${t.label}") },
                            colors = FilterChipDefaults.filterChipColors(
                                containerColor = Glass.GlassFill,
                                selectedContainerColor = Glass.AccentGlow.copy(alpha = 0.35f),
                                labelColor = Glass.InkDim,
                                selectedLabelColor = Glass.Ink,
                            ),
                        )
                    }
                }
            }
        }

        Column {
            if (created == null) {
                GlowButton("Create my identity", Modifier.fillMaxWidth()) {
                    created = onCreate(tier)
                }
            } else {
                GlowButton("Enter AetherNet", Modifier.fillMaxWidth()) {
                    onDone(created!!)
                }
            }
            Spacer(Modifier.height(30.dp))
        }
    }
}
