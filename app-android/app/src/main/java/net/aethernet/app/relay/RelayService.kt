package net.aethernet.app.relay

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import net.aethernet.app.NetGraph
import net.aethernet.app.R
import net.aethernet.app.data.MessageStore
import net.aethernet.app.ui.MainActivity

/**
 * AetherNet relay service — the phone *is* the tower.
 *
 * Runs as a dataSync foreground service: beacons the LAN so other nodes see
 * this device, receives beacons to learn the peer table, delivers inbound
 * messages into the conversation store, and drains the spool (messages parked
 * while a peer was offline) the moment that peer is sighted again —
 * store-and-forward, the mesh property that makes AetherNet work across gaps
 * in coverage.
 */
class RelayService : Service() {

    companion object {
        const val CHANNEL_ID = "aethernet_relay"
        const val NOTIFICATION_ID = 4242

        fun start(context: Context) {
            val intent = Intent(context, RelayService::class.java)
            if (Build.VERSION.SDK_INT >= 26) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, RelayService::class.java))
        }
    }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    override fun onCreate() {
        super.onCreate()
        NetGraph.init(this)
        startInForeground()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val id = NetGraph.identity ?: run {
            stopSelf()
            return START_NOT_STICKY
        }
        val name = NetGraph.handle ?: id.number.display
        val transport = NetGraph.ensureTransport(this, name) ?: run {
            stopSelf()
            return START_NOT_STICKY
        }
        if (!transport.running.value) transport.start()
        collectEvents(transport)
        return START_STICKY
    }

    private fun collectEvents(transport: net.aethernet.app.net.LanTransport) {
        scope.launch {
            transport.events.collectLatest { event ->
                when (event) {
                    is net.aethernet.app.net.LanTransport.Event.Message -> {
                        val store = NetGraph.messages
                        store.ensureConversation(event.fromNumber)
                        val known = store.messages(event.fromNumber)
                        if (known.none { it.id == event.msgId }) {
                            store.append(
                                event.fromNumber,
                                MessageStore.Msg(
                                    id = event.msgId,
                                    ts = event.ts,
                                    outgoing = false,
                                    text = event.text,
                                    status = MessageStore.Status.DELIVERED,
                                )
                            )
                        }
                        drainSpool(transport, event.fromNumber)
                    }
                    is net.aethernet.app.net.LanTransport.Event.PeerSeen -> {
                        drainSpool(transport, event.peer.number)
                    }
                }
            }
        }
    }

    /** Deliver every parked message for a peer now that they are reachable. */
    private fun drainSpool(transport: net.aethernet.app.net.LanTransport, number: String) {
        val pending = NetGraph.messages.pendingFor(number)
        for (msg in pending) {
            val res = transport.sendText(number, msg.text)
            if (res is net.aethernet.app.net.LanTransport.SendResult.Sent) {
                NetGraph.messages.markDelivered(number, msg.id)
            }
        }
    }

    private fun startInForeground() {
        val manager = getSystemService(NOTIFICATION_SERVICE) as NotificationManager
        if (Build.VERSION.SDK_INT >= 26) {
            manager.createNotificationChannel(
                NotificationChannel(
                    CHANNEL_ID,
                    "AetherNet relay",
                    NotificationManager.IMPORTANCE_LOW,
                ).apply {
                    description = "Keeps the AetherNet relay alive"
                    setShowBadge(false)
                }
            )
        }
        val tap = PendingIntent.getActivity(
            this, 0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val notification: Notification =
            NotificationCompat.Builder(this, CHANNEL_ID)
                .setSmallIcon(R.drawable.ic_launcher_fg)
                .setContentTitle("AetherNet relay is live")
                .setContentText("Your phone is a node. Messages route through you, end-to-end encrypted.")
                .setOngoing(true)
                .setContentIntent(tap)
                .build()
        if (Build.VERSION.SDK_INT >= 34) {
            startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
    }

    override fun onDestroy() {
        NetGraph.transport?.stop()
        scope.cancel()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null
}
