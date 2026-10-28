package net.aethernet.app.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

/**
 * AetherNet Liquid Glass palette.
 *
 * Deep space navy field, frosted glass surfaces, and the icy blue accent the
 * user picked (#B3E5FC) with a cyan glow companion (#4FC3F7). Colorful ambient
 * blobs behind the glass echo the iOS-26 Liquid Glass references.
 */
object Glass {
    // Field
    val FieldTop = Color(0xFF0B1222)
    val FieldBottom = Color(0xFF070B14)

    // Accent (picked from the reference: icy blue)
    val Accent = Color(0xFFB3E5FC)
    val AccentGlow = Color(0xFF4FC3F7)
    val AccentDeep = Color(0xFF0288D1)

    // Ambient blob colors (liquid reflections)
    val BlobRose = Color(0xFFE0447C)
    val BlobViolet = Color(0xFF6C4DE0)
    val BlobTeal = Color(0xFF1FB6A6)
    val BlobAmber = Color(0xFFE0A83C)

    // Glass surfaces
    val GlassFill = Color.White.copy(alpha = 0.08f)
    val GlassFillStrong = Color.White.copy(alpha = 0.14f)
    val GlassBorder = Color.White.copy(alpha = 0.22f)
    val GlassSpecular = Color.White.copy(alpha = 0.16f)

    // Text
    val Ink = Color(0xFFEAF4FF)
    val InkDim = Color(0xFF9FB6CC)
    val InkFaint = Color(0xFF6B8199)

    val BubbleOut = Color(0xFF17475E).copy(alpha = 0.55f)
    val BubbleIn = Color.White.copy(alpha = 0.08f)

    val Danger = Color(0xFFFF6B6B)
    val Ok = Color(0xFF4ADE80)

    val fieldBrush = Brush.verticalGradient(listOf(FieldTop, FieldBottom))

    val specularBrush = Brush.verticalGradient(
        0f to GlassSpecular,
        0.5f to Color.Transparent,
        startY = 0f,
        endY = Float.POSITIVE_INFINITY,
    )
}

val GlassTypography = Typography(
    headlineLarge = TextStyle(
        fontFamily = FontFamily.SansSerif, fontWeight = FontWeight.Bold,
        fontSize = 34.sp, lineHeight = 40.sp, color = Glass.Ink,
    ),
    headlineMedium = TextStyle(
        fontFamily = FontFamily.SansSerif, fontWeight = FontWeight.Bold,
        fontSize = 26.sp, lineHeight = 32.sp, color = Glass.Ink,
    ),
    titleLarge = TextStyle(
        fontFamily = FontFamily.SansSerif, fontWeight = FontWeight.SemiBold,
        fontSize = 20.sp, lineHeight = 26.sp, color = Glass.Ink,
    ),
    titleMedium = TextStyle(
        fontFamily = FontFamily.SansSerif, fontWeight = FontWeight.SemiBold,
        fontSize = 16.sp, lineHeight = 22.sp, color = Glass.Ink,
    ),
    bodyLarge = TextStyle(
        fontFamily = FontFamily.SansSerif, fontWeight = FontWeight.Normal,
        fontSize = 16.sp, lineHeight = 23.sp, color = Glass.Ink,
    ),
    bodyMedium = TextStyle(
        fontFamily = FontFamily.SansSerif, fontWeight = FontWeight.Normal,
        fontSize = 14.sp, lineHeight = 20.sp, color = Glass.InkDim,
    ),
    bodySmall = TextStyle(
        fontFamily = FontFamily.SansSerif, fontWeight = FontWeight.Normal,
        fontSize = 12.sp, lineHeight = 17.sp, color = Glass.InkFaint,
    ),
    labelLarge = TextStyle(
        fontFamily = FontFamily.SansSerif, fontWeight = FontWeight.SemiBold,
        fontSize = 14.sp, color = Glass.Ink,
    ),
)

@Composable
fun AetherNetTheme(content: @Composable () -> Unit) {
    // The app is a night-sky surface by design; dark palette always.
    @Suppress("UNUSED_EXPRESSION") isSystemInDarkTheme()
    MaterialTheme(
        colorScheme = MaterialTheme.colorScheme.copy(
            primary = Glass.AccentGlow,
            onPrimary = Color(0xFF06202E),
            background = Glass.FieldBottom,
            onBackground = Glass.Ink,
            surface = Glass.FieldTop,
            onSurface = Glass.Ink,
            surfaceVariant = Glass.GlassFill,
            onSurfaceVariant = Glass.InkDim,
            outline = Glass.GlassBorder,
        ),
        typography = GlassTypography,
        content = content,
    )
}
