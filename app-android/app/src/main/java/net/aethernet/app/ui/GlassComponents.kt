package net.aethernet.app.ui

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Liquid Glass components.
 *
 * True backdrop blur is API-31+; the design instead layers translucent fills,
 * a specular top sheen, hairline borders, and slow-drifting ambient blobs —
 * which is what actually reads as "liquid glass" on the reference boards.
 */

/** Drifting colorful blobs behind the glass; the "liquid" under the glass. */
@Composable
fun LiquidBackground(modifier: Modifier = Modifier) {
    val transition = rememberInfiniteTransition(label = "liquid")
    val t by transition.animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(tween(16000, easing = LinearEasing), RepeatMode.Reverse),
        label = "drift",
    )
    val t2 by transition.animateFloat(
        initialValue = 1f,
        targetValue = 0f,
        animationSpec = infiniteRepeatable(tween(21000, easing = LinearEasing), RepeatMode.Reverse),
        label = "drift2",
    )
    Box(
        modifier
            .fillMaxSize()
            .background(Glass.fieldBrush)
    ) {
        Blob(Glass.BlobRose, size = 280.dp, center = Offset(0.15f + 0.1f * t, 0.12f + 0.05f * t2), alpha = 0.30f)
        Blob(Glass.BlobViolet, size = 340.dp, center = Offset(0.88f - 0.08f * t2, 0.22f + 0.06f * t), alpha = 0.26f)
        Blob(Glass.BlobTeal, size = 300.dp, center = Offset(0.20f + 0.12f * t2, 0.86f - 0.05f * t), alpha = 0.24f)
        Blob(Glass.AccentGlow, size = 260.dp, center = Offset(0.85f - 0.06f * t, 0.85f - 0.08f * t2), alpha = 0.22f)
    }
}

@Composable
private fun Blob(color: Color, size: Dp, center: Offset, alpha: Float) {
    Box(
        Modifier
            .fillMaxSize()
    ) {
        Box(
            Modifier
                .graphicsLayer {
                    translationX = center.x * this.size.width - size.toPx() / 2
                    translationY = center.y * this.size.height - size.toPx() / 2
                    this.alpha = alpha
                }
                .size(size)
                .clip(CircleShape)
                .background(
                    Brush.radialGradient(
                        colors = listOf(color.copy(alpha = 0.9f), Color.Transparent),
                        radius = size.value * 3f,
                    )
                )
        )
    }
}

/** Frosted glass panel: translucent fill + specular sheen + hairline border. */
@Composable
fun GlassCard(
    modifier: Modifier = Modifier,
    corner: Dp = 24.dp,
    fill: Color = Glass.GlassFill,
    content: @Composable androidx.compose.foundation.layout.ColumnScope.() -> Unit,
) {
    Column(
        modifier = modifier
            .clip(RoundedCornerShape(corner))
            .background(
                Brush.verticalGradient(
                    listOf(fill, fill.copy(alpha = fill.alpha * 0.6f))
                )
            )
            .border(1.dp, Brush.verticalGradient(listOf(Glass.GlassBorder, Color.White.copy(alpha = 0.06f))), RoundedCornerShape(corner))
            .then(
                Modifier.background(Glass.specularBrush)
            ),
        content = content,
    )
}

/** Primary action button in the icy accent. */
@Composable
fun GlowButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    height: Dp = 54.dp,
) {
    Button(
        onClick = onClick,
        enabled = enabled,
        shape = RoundedCornerShape(18.dp),
        colors = ButtonDefaults.buttonColors(
            containerColor = Glass.AccentGlow.copy(alpha = 0.9f),
            contentColor = Color(0xFF06202E),
            disabledContainerColor = Glass.GlassFillStrong,
            disabledContentColor = Glass.InkFaint,
        ),
        modifier = modifier.height(height),
    ) {
        Text(text, fontWeight = FontWeight.SemiBold, fontSize = 16.sp)
    }
}

/** Tier badge: STANDARD / ELITE / PRESTIGE. */
@Composable
fun TierBadge(tier: net.aethernet.app.Tier, modifier: Modifier = Modifier) {
    val color = when (tier) {
        net.aethernet.app.Tier.STANDARD -> Glass.AccentGlow
        net.aethernet.app.Tier.ELITE -> Glass.BlobAmber
        net.aethernet.app.Tier.PRESTIGE -> Glass.BlobRose
    }
    Box(
        modifier
            .clip(RoundedCornerShape(999.dp))
            .background(color.copy(alpha = 0.18f))
            .border(1.dp, color.copy(alpha = 0.55f), RoundedCornerShape(999.dp))
            .padding(horizontal = 10.dp, vertical = 3.dp)
    ) {
        Text(
            tier.label.uppercase(),
            color = color,
            fontSize = 10.sp,
            fontWeight = FontWeight.Bold,
            letterSpacing = 1.5.sp,
        )
    }
}

/** Big mono-spaced number display used on the identity card. */
@Composable
fun NumberDisplay(
    number: net.aethernet.app.AetherNumber,
    fontSize: TextUnit = 30.sp,
    modifier: Modifier = Modifier,
) {
    Text(
        number.display,
        modifier = modifier,
        fontSize = fontSize,
        fontWeight = FontWeight.Bold,
        letterSpacing = 2.sp,
        color = Glass.Accent,
    )
}

/** Small status dot + label row used for relay/connection state. */
@Composable
fun StatusDot(on: Boolean, label: String, modifier: Modifier = Modifier) {
    Row(modifier, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Box(
            Modifier
                .size(8.dp)
                .clip(CircleShape)
                .background(if (on) Glass.Ok else Glass.InkFaint)
        )
        Text(label, style = GlassTypography.bodySmall)
    }
}
