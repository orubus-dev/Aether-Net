package net.aethernet.app.ui

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import net.aethernet.app.Identity
import net.aethernet.app.NetGraph
import net.aethernet.app.Tier
import net.aethernet.app.relay.RelayService

/**
 * AetherNet — single-activity Compose app.
 *
 * Flow: no identity on disk -> Onboarding (create keypair + derive number)
 * -> Home (chats / numbers / sites / settings). The relay service starts
 * automatically once an identity exists and notifications are granted, so the
 * phone joins the mesh the moment the app has run once.
 */
class MainActivity : ComponentActivity() {

    private val notifPermission =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
            if (granted && NetGraph.identity != null) RelayService.start(this)
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        NetGraph.init(this)
        setContent {
            AetherNetTheme {
                Surface(Modifier.fillMaxSize(), color = Glass.FieldBottom) {
                    App()
                }
            }
        }
    }

    @Composable
    private fun App() {
        var identity by remember { mutableStateOf(NetGraph.identity) }
        var handle by remember { mutableStateOf(NetGraph.handle) }

        // After onboarding completes: persist relay start + notification grant.
        fun activate(id: Identity, h: String?) {
            identity = id
            handle = h
            requestNotificationsThenRelay()
        }

        when (val id = identity) {
            null -> Onboarding(
                onCreate = { tier -> NetGraph.createIdentity(tier) },
                onDone = { created -> activate(created, NetGraph.handle) },
            )
            else -> {
                LaunchedEffect(id) { RelayService.start(this@MainActivity) }
                Home(id, handle)
            }
        }
    }

    private fun requestNotificationsThenRelay() {
        if (Build.VERSION.SDK_INT >= 33) {
            val granted = checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) ==
                PackageManager.PERMISSION_GRANTED
            if (!granted) {
                notifPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
            } else {
                RelayService.start(this)
            }
        } else {
            RelayService.start(this)
        }
    }
}
