package net.aethernet.app

import android.content.Context
import net.aethernet.app.data.IdentityStore
import net.aethernet.app.data.MessageStore
import net.aethernet.app.net.LanTransport

/**
 * Process-wide service locator. Both the Activity and RelayService resolve
 * the same identity/store/transport instances through this object.
 */
object NetGraph {

    @Volatile var identity: Identity? = null
    @Volatile var handle: String? = null

    /** Human profile fields surfaced in the registry record. */
    @Volatile var displayName: String? = null
    @Volatile var emailPointer: String? = null

    /** Assist endpoint ("GitHub as the VPS" pipe). Override in Settings. */
    @Volatile var assistEndpoint: String = "https://aether-net.vercel.app"

    lateinit var identityStore: IdentityStore
        private set
    lateinit var messages: MessageStore
        private set

    @Volatile var transport: LanTransport? = null

    fun init(context: Context) {
        if (!this::identityStore.isInitialized) {
            identityStore = IdentityStore(context.applicationContext)
            messages = MessageStore(context.applicationContext)
            val loaded = identityStore.load()
            identity = loaded?.first
            handle = loaded?.second
            val profile = identityStore.loadProfile()
            displayName = profile.first
            emailPointer = profile.second
            assistEndpoint = context.getSharedPreferences("aethernet", Context.MODE_PRIVATE)
                .getString("assist_endpoint", "https://aether-net.vercel.app")
                ?: "https://aether-net.vercel.app"
        }
    }

    /** Update the profile and persist it with the identity. */
    fun setProfile(name: String?, email: String?) {
        displayName = name?.ifBlank { null }
        emailPointer = email?.ifBlank { null }
        identityStore.saveProfile(displayName, emailPointer)
    }

    /** Point the assist layer at a different endpoint (your sandbox VPS). */
    fun setAssistEndpoint(url: String, context: Context) {
        assistEndpoint = url.trim().removeSuffix("/")
        context.getSharedPreferences("aethernet", Context.MODE_PRIVATE)
            .edit().putString("assist_endpoint", assistEndpoint).apply()
    }

    /** Create a brand-new identity and persist it immediately. */
    fun createIdentity(tier: Tier): Identity {
        val id = Identity.generate(tier)
        identity = id
        handle = null
        identityStore.save(id, null)
        return id
    }

    /** Bump the derivation counter after a registry collision. */
    fun rederiveWithCounter(newCounter: Long): Identity? {
        val current = identity ?: return null
        identityStore.updateCounter(current, newCounter, handle)
        identity = Identity.fromStored(current.seed, newCounter, current.tier)
        return identity
    }

    fun updateHandle(h: String?) {
        handle = h
        identity?.let { identityStore.save(it, h) }
    }

    fun ensureTransport(context: Context, name: String): LanTransport? {
        val id = identity ?: return null
        if (transport == null) {
            transport = LanTransport(context.applicationContext, id, name)
        }
        return transport
    }
}
