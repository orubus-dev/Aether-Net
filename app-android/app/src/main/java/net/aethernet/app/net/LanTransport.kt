package net.aethernet.app.net

import android.content.Context
import android.net.wifi.WifiManager
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import net.aethernet.app.Crypto
import net.aethernet.app.Identity
import org.json.JSONObject
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.SocketTimeoutException
import java.security.SecureRandom
import java.util.concurrent.ConcurrentHashMap

/**
 * Talk-layer v0 transport over Wi-Fi.
 *
 * Wire format (docs/talk-layer-v0.md):
 *   beacon: "AEN1" | senderPubKey[32] | nameLen[1] | name
 *   data:   "AEN2" | senderPubKey[32] | nonce[24] | XChaCha20-Poly1305-IETF(
 *           plaintext JSON, AD = "AEN2"|senderPk|recipientPk, key =
 *           crypto_box_beforenm(peerCurvePk, myCurveSk))
 *
 * Discovery is LAN broadcast on UDP 42428. Every node beacons ~every 4s and
 * every received beacon feeds the peer table. Data packets are answered from
 * the peer table; a peer we have not heard from is unreachable, so the sender
 * parks the message as PENDING in the spool and the relay service redelivers
 * on the next sighting (store-and-forward, hop 1 of the mesh).
 */
class LanTransport(
    context: Context,
    private val identity: Identity,
    private val displayName: String,
) {
    companion object {
        const val PORT = 42428
        const val MAGIC_BEACON = "AEN1"
        const val MAGIC_DATA = "AEN2"
        const val MAX_PACKET = 4096
    }

    data class Peer(
        val publicKey: ByteArray,
        val number: String,
        val name: String,
        var address: InetAddress,
        var lastSeen: Long,
    ) {
        override fun equals(other: Any?) = other is Peer && publicKey.contentEquals(other.publicKey)
        override fun hashCode() = publicKey.contentHashCode()
    }

    sealed class Event {
        data class PeerSeen(val peer: Peer) : Event()
        data class Message(
            val fromNumber: String,
            val fromName: String,
            val msgId: String,
            val ts: Long,
            val text: String,
        ) : Event()
    }

    private val peers = ConcurrentHashMap<String, Peer>() // number -> peer
    private val random = SecureRandom()

    private val _events = MutableSharedFlow<Event>(
        replay = 0, extraBufferCapacity = 64, onBufferOverflow = BufferOverflow.DROP_OLDEST
    )
    val events: SharedFlow<Event> = _events

    private val _running = MutableStateFlow(false)
    val running: StateFlow<Boolean> = _running

    private var socket: DatagramSocket? = null
    private var multicastLock: WifiManager.MulticastLock? = null
    private var listenThread: Thread? = null
    private var beaconThread: Thread? = null
    @Volatile private var stop = false

    /** Outbound queue for the beacon name so the UI can rename live. */
    @Volatile var beaconName: String = displayName

    fun peerTable(): List<Peer> = peers.values.sortedByDescending { it.lastSeen }

    fun peerByNumber(number: String): Peer? = peers[number]

    private fun newNonce(): ByteArray = ByteArray(Crypto.NONCE_BYTES).also { random.nextBytes(it) }

    fun start() {
        if (_running.value) return
        stop = false
        val wifi = context().applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
        multicastLock = wifi.createMulticastLock("aethernet-relay").apply {
            setReferenceCounted(false)
            acquire()
        }
        val s = DatagramSocket(null).apply {
            reuseAddress = true
            broadcast = true
            bind(InetSocketAddress(PORT))
        }
        socket = s
        listenThread = Thread({ listenLoop(s) }, "aen-listen").apply { start() }
        beaconThread = Thread({ beaconLoop(s) }, "aen-beacon").apply { start() }
        _running.value = true
    }

    fun stop() {
        stop = true
        runCatching { socket?.close() }
        runCatching { multicastLock?.release() }
        socket = null
        _running.value = false
    }

    private fun listenLoop(s: DatagramSocket) {
        val buf = ByteArray(MAX_PACKET)
        while (!stop && !Thread.currentThread().isInterrupted) {
            try {
                s.soTimeout = 1000
                val p = DatagramPacket(buf, buf.size)
                s.receive(p)
                handle(buf, p.length, p.address)
            } catch (_: SocketTimeoutException) {
                // expected tick
            } catch (_: Exception) {
                if (stop) return
            }
        }
    }

    private fun handle(raw: ByteArray, len: Int, from: InetAddress) {
        if (len < 4) return
        val magic = String(raw, 0, 4, Charsets.US_ASCII)
        when (magic) {
            MAGIC_BEACON -> if (len >= 4 + Crypto.PUB_BYTES + 1) handleBeacon(raw, len, from)
            MAGIC_DATA -> if (len >= 4 + Crypto.PUB_BYTES + Crypto.NONCE_BYTES + Crypto.MAC_BYTES) {
                handleData(raw, len)
            }
        }
    }

    private fun handleBeacon(raw: ByteArray, len: Int, from: InetAddress) {
        val pub = raw.copyOfRange(4, 4 + Crypto.PUB_BYTES)
        if (pub.contentEquals(identity.publicKey)) return // self
        val nameLen = raw[4 + Crypto.PUB_BYTES].toInt() and 0xFF
        val nameEnd = 4 + Crypto.PUB_BYTES + 1 + nameLen
        if (len < nameEnd) return
        val name = String(raw, 4 + Crypto.PUB_BYTES + 1, nameLen, Charsets.UTF_8)
        val number = net.aethernet.app.AetherNumber.derive(pub, 0).plain
        val peer = peers.getOrPut(number) {
            Peer(pub, number, name, from, System.currentTimeMillis())
        }
        peer.address = from
        peer.name = name
        peer.lastSeen = System.currentTimeMillis()
        _events.tryEmit(Event.PeerSeen(peer))
    }

    private fun handleData(raw: ByteArray, len: Int) {
        val senderPk = raw.copyOfRange(4, 4 + Crypto.PUB_BYTES)
        val nonce = raw.copyOfRange(4 + Crypto.PUB_BYTES, 4 + Crypto.PUB_BYTES + Crypto.NONCE_BYTES)
        val ct = raw.copyOfRange(4 + Crypto.PUB_BYTES + Crypto.NONCE_BYTES, len)
        val recipPk = identity.publicKey
        val ad = adFor(senderPk, recipPk)
        val key = Identity.keyToPeer(identity, senderPk)
        val plain = Crypto.unseal(ct, ad, nonce, key) ?: return // wrong key / tampered
        runCatching {
            val o = JSONObject(String(plain, Charsets.UTF_8))
            val fromNumber = net.aethernet.app.AetherNumber.derive(senderPk, 0).plain
            _events.tryEmit(
                Event.Message(
                    fromNumber = fromNumber,
                    fromName = peers[fromNumber]?.name ?: fromNumber,
                    msgId = o.getString("id"),
                    ts = o.getLong("ts"),
                    text = o.getString("text"),
                )
            )
        }
    }

    private fun adFor(senderPk: ByteArray, recipPk: ByteArray): ByteArray =
        MAGIC_DATA.toByteArray(Charsets.US_ASCII) + senderPk + recipPk

    private fun beaconLoop(s: DatagramSocket) {
        while (!stop) {
            sendBeacon(s)
            try {
                Thread.sleep(4000)
            } catch (_: InterruptedException) {
                return
            }
        }
    }

    private fun sendBeacon(s: DatagramSocket) {
        runCatching {
            val name = beaconName.toByteArray(Charsets.UTF_8).let {
                if (it.size > 60) it.copyOf(60) else it
            }
            val out = MAGIC_BEACON.toByteArray(Charsets.US_ASCII) +
                identity.publicKey + byteArrayOf(name.size.toByte()) + name
            val dst = InetAddress.getByName("255.255.255.255")
            s.send(DatagramPacket(out, out.size, dst, PORT))
        }
    }

    /**
     * Send an encrypted text message to a peer by number. Returns true when
     * the packet left the socket to a known address; false when the peer is
     * unknown (caller parks the message as PENDING in the spool).
     */
    fun sendText(toNumber: String, text: String): SendResult = runCatching {
        val s = socket ?: return@runCatching SendResult.NO_PEER
        val peer = peers[toNumber] ?: return@runCatching SendResult.NO_PEER
        val msgId = randomHex(16)
        val body = JSONObject()
            .put("id", msgId)
            .put("ts", System.currentTimeMillis())
            .put("text", text)
        val plain = body.toString().toByteArray(Charsets.UTF_8)
        val nonce = newNonce()
        val ad = adFor(identity.publicKey, peer.publicKey)
        val key = Identity.keyToPeer(identity, peer.publicKey)
        val ct = Crypto.seal(plain, ad, nonce, key)
        val out = MAGIC_DATA.toByteArray(Charsets.US_ASCII) +
            identity.publicKey + nonce + ct
        s.send(DatagramPacket(out, out.size, peer.address, PORT))
        SendResult.Sent(msgId)
    }.getOrElse { SendResult.NO_PEER }

    sealed class SendResult {
        data class Sent(val msgId: String) : SendResult()
        object NO_PEER : SendResult()
    }

    private fun randomHex(n: Int): String {
        val b = ByteArray(n).also { random.nextBytes(it) }
        return b.joinToString("") { "%02x".format(it) }
    }

    /** Context holder kept private; transport is constructed with it. */
    private val appContext: Context = context.applicationContext
    private fun context(): Context = appContext
}
