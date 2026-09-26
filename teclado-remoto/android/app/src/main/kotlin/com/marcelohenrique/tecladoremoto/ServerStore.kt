package com.marcelohenrique.tecladoremoto

import android.content.Context
import android.os.Build
import android.provider.Settings
import com.marcelohenrique.tecladoremoto.core.Crypto
import com.marcelohenrique.tecladoremoto.core.DeviceIdentity
import org.json.JSONArray
import org.json.JSONObject

class SavedServer(val id: String, val name: String, val host: String, val port: Int, val key: ByteArray)

/** PCs pareados e preferências, no armazenamento privado do app. */
class ServerStore(context: Context) {
    private val prefs = context.getSharedPreferences("teclado_remoto", Context.MODE_PRIVATE)
    private val resolver = context.applicationContext.contentResolver

    val identity: DeviceIdentity by lazy { DeviceIdentity(deviceId(), deviceName()) }

    private fun deviceId(): ByteArray {
        prefs.getString("device_id", null)?.let(Crypto::unhex)?.takeIf { it.size == 16 }?.let { return it }
        val id = Crypto.randomBytes(16)
        prefs.edit().putString("device_id", Crypto.hex(id)).apply()
        return id
    }

    private fun deviceName(): String {
        val custom = if (Build.VERSION.SDK_INT >= 25) {
            try {
                Settings.Global.getString(resolver, Settings.Global.DEVICE_NAME)
            } catch (_: Exception) {
                null
            }
        } else {
            null
        }
        if (!custom.isNullOrBlank()) return custom
        val maker = Build.MANUFACTURER.orEmpty().replaceFirstChar { it.uppercaseChar() }
        val model = Build.MODEL.orEmpty()
        return if (model.startsWith(maker, ignoreCase = true)) model else "$maker $model".trim()
    }

    fun servers(): List<SavedServer> {
        val array = try {
            JSONArray(prefs.getString("servers", "[]"))
        } catch (_: Exception) {
            JSONArray()
        }
        return (0 until array.length()).mapNotNull { i ->
            val o = array.optJSONObject(i) ?: return@mapNotNull null
            val key = Crypto.unhex(o.optString("key"))?.takeIf { it.size == 32 } ?: return@mapNotNull null
            SavedServer(o.optString("id"), o.optString("name"), o.optString("host"), o.optInt("port"), key)
        }
    }

    fun find(id: String) = servers().firstOrNull { it.id == id }

    fun save(server: SavedServer) {
        val list = servers().filter { it.id != server.id } + server
        val array = JSONArray()
        for (s in list) {
            array.put(
                JSONObject()
                    .put("id", s.id)
                    .put("name", s.name)
                    .put("host", s.host)
                    .put("port", s.port)
                    .put("key", Crypto.hex(s.key)),
            )
        }
        prefs.edit().putString("servers", array.toString()).apply()
    }

    fun remove(id: String) {
        val remaining = servers().filter { it.id != id }
        prefs.edit().putString("servers", "[]").apply()
        remaining.forEach(::save)
        if (lastServerId == id) lastServerId = null
    }

    var lastServerId: String?
        get() = prefs.getString("last_server", null)
        set(value) = prefs.edit().putString("last_server", value).apply()

    var keepScreenOn: Boolean
        get() = prefs.getBoolean("keep_screen_on", true)
        set(value) = prefs.edit().putBoolean("keep_screen_on", value).apply()

    var haptics: Boolean
        get() = prefs.getBoolean("haptics", true)
        set(value) = prefs.edit().putBoolean("haptics", value).apply()

    var volumeKeys: Boolean
        get() = prefs.getBoolean("volume_keys", false)
        set(value) = prefs.edit().putBoolean("volume_keys", value).apply()

    var inputMode: Int
        get() = prefs.getInt("input_mode", 0)
        set(value) = prefs.edit().putInt("input_mode", value).apply()
}
