package com.marcelohenrique.tecladoremoto.core

import java.io.ByteArrayOutputStream

/** Protocolo TRK1 — mesmo formato do servidor do PC (docs/PROTOCOLO.md). */
object Proto {
    const val TCP_PORT = 47800
    const val DISCOVERY_PORT = 47810
    val MAGIC = byteArrayOf('T'.code.toByte(), 'R'.code.toByte(), 'K'.code.toByte(), '1'.code.toByte())
    const val VERSION = 1

    const val MODE_SESSION = 1
    const val MODE_PAIR = 2

    const val ST_OK = 0
    const val ST_NOT_PAIRED = 1
    const val ST_PAIRING_DISABLED = 2
    const val ST_BUSY = 3
    const val ST_BAD_VERSION = 4
    const val ST_REJECTED = 5
    const val ST_BAD_REQUEST = 6
    const val ST_TIMEOUT = 7

    const val ID_LEN = 16
    const val PUB_LEN = 65
    const val NONCE_LEN = 32
    const val MAX_NAME = 64
    const val MAX_PLAINTEXT = 1 shl 20
    const val TAG_LEN = 16

    const val MSG_TYPE = 0x01
    const val MSG_KEY = 0x02
    const val MSG_CHAR = 0x03
    const val MSG_RELEASE_ALL = 0x04
    const val MSG_CLIP_SET = 0x10
    const val MSG_CLIP_GET = 0x11
    const val MSG_CLIP_PASTE = 0x12
    const val MSG_MOUSE_MOVE = 0x20
    const val MSG_MOUSE_BUTTON = 0x21
    const val MSG_MOUSE_WHEEL = 0x22
    const val MSG_ACTION = 0x30
    const val MSG_PING = 0x40
    const val MSG_BYE = 0x41

    const val MSG_WELCOME = 0x80
    const val MSG_PONG = 0x81
    const val MSG_CLIP_DATA = 0x82
    const val MSG_NOTICE = 0x83

    const val KEY_TAP = 0
    const val KEY_DOWN = 1
    const val KEY_UP = 2

    const val BUTTON_LEFT = 1
    const val BUTTON_RIGHT = 2
    const val BUTTON_MIDDLE = 3

    const val ACTION_LOCK = 1
    const val WELCOME_FLAG_ELEVATED = 1

    /** Maior texto que cabe numa mensagem (o resto é cortado sem quebrar caractere). */
    const val MAX_TEXT_BYTES = MAX_PLAINTEXT - 8

    fun clientHello(mode: Int, deviceId: ByteArray, public: ByteArray, nonce: ByteArray, name: String): ByteArray {
        require(deviceId.size == ID_LEN && public.size == PUB_LEN && nonce.size == NONCE_LEN)
        val nameBytes = utf8Prefix(name, MAX_NAME)
        return ByteArrayOutputStream(120 + nameBytes.size).apply {
            write(MAGIC)
            write(VERSION)
            write(mode)
            write(deviceId)
            write(public)
            write(nonce)
            write(nameBytes.size)
            write(nameBytes)
        }.toByteArray()
    }

    fun type(backspaces: Int, text: String): ByteArray {
        val bytes = utf8Prefix(text, MAX_TEXT_BYTES)
        val n = backspaces.coerceIn(0, 0xFFFF)
        return byteArrayOf(MSG_TYPE.toByte(), (n shr 8).toByte(), n.toByte()) + bytes
    }

    fun key(action: Int, mods: Int, vk: Int): ByteArray =
        byteArrayOf(MSG_KEY.toByte(), action.toByte(), mods.toByte(), (vk shr 8).toByte(), vk.toByte())

    fun char(mods: Int, codepoint: Int): ByteArray = byteArrayOf(
        MSG_CHAR.toByte(), mods.toByte(),
        (codepoint shr 24).toByte(), (codepoint shr 16).toByte(), (codepoint shr 8).toByte(), codepoint.toByte(),
    )

    fun releaseAll() = byteArrayOf(MSG_RELEASE_ALL.toByte())
    fun clipSet(text: String) = byteArrayOf(MSG_CLIP_SET.toByte()) + utf8Prefix(text, MAX_TEXT_BYTES)
    fun clipGet() = byteArrayOf(MSG_CLIP_GET.toByte())
    fun clipPaste(text: String) = byteArrayOf(MSG_CLIP_PASTE.toByte()) + utf8Prefix(text, MAX_TEXT_BYTES)

    fun mouseMove(dx: Int, dy: Int): ByteArray {
        val x = dx.coerceIn(-32768, 32767)
        val y = dy.coerceIn(-32768, 32767)
        return byteArrayOf(MSG_MOUSE_MOVE.toByte(), (x shr 8).toByte(), x.toByte(), (y shr 8).toByte(), y.toByte())
    }

    fun mouseButton(button: Int, action: Int) = byteArrayOf(MSG_MOUSE_BUTTON.toByte(), button.toByte(), action.toByte())

    fun mouseWheel(vertical: Int, horizontal: Int): ByteArray {
        val v = vertical.coerceIn(-32768, 32767)
        val h = horizontal.coerceIn(-32768, 32767)
        return byteArrayOf(MSG_MOUSE_WHEEL.toByte(), (v shr 8).toByte(), v.toByte(), (h shr 8).toByte(), h.toByte())
    }

    fun action(id: Int) = byteArrayOf(MSG_ACTION.toByte(), id.toByte())

    fun ping(stamp: Long) = ByteArray(9).also {
        it[0] = MSG_PING.toByte()
        for (i in 0 until 8) it[1 + i] = (stamp ushr (56 - 8 * i)).toByte()
    }

    fun bye() = byteArrayOf(MSG_BYE.toByte())

    fun readLong(b: ByteArray, offset: Int): Long {
        var v = 0L
        for (i in 0 until 8) v = (v shl 8) or (b[offset + i].toLong() and 0xFF)
        return v
    }

    fun text(b: ByteArray, offset: Int): String = String(b, offset, b.size - offset, Charsets.UTF_8)

    /** Os primeiros bytes UTF-8 de [s] que cabem em [max], sem cortar um caractere. */
    fun utf8Prefix(s: String, max: Int): ByteArray {
        val bytes = s.toByteArray(Charsets.UTF_8)
        if (bytes.size <= max) return bytes
        var end = max
        while (end > 0 && (bytes[end].toInt() and 0xC0) == 0x80) end--
        return bytes.copyOf(end)
    }
}

/** Resposta do PC ao CLIENT_HELLO. */
class ServerHello(
    val raw: ByteArray,
    val status: Int,
    val serverId: ByteArray,
    val public: ByteArray,
    val nonceOrCommit: ByteArray,
    val name: String,
) {
    companion object {
        fun parse(b: ByteArray): ServerHello {
            if (b.size < 6 || !b.copyOfRange(0, 4).contentEquals(Proto.MAGIC)) {
                throw ProtocolException("O PC respondeu algo que não é o Teclado Remoto.")
            }
            val status = b[5].toInt() and 0xFF
            if (status != Proto.ST_OK) return ServerHello(b, status, ByteArray(0), ByteArray(0), ByteArray(0), "")
            if (b.size < 120) throw ProtocolException("Resposta do PC incompleta.")
            val nameLen = b[119].toInt() and 0xFF
            if (b.size != 120 + nameLen) throw ProtocolException("Resposta do PC inválida.")
            return ServerHello(
                raw = b,
                status = status,
                serverId = b.copyOfRange(6, 22),
                public = b.copyOfRange(22, 87),
                nonceOrCommit = b.copyOfRange(87, 119),
                name = String(b, 120, nameLen, Charsets.UTF_8),
            )
        }
    }
}

open class ProtocolException(message: String) : java.io.IOException(message)

class WrongServerException : ProtocolException("Outro computador respondeu neste endereço.")

/** O PC recusou a conexão com um status do protocolo (não pareado, recusado...). */
class RefusedException(val status: Int) : ProtocolException(describe(status)) {
    companion object {
        fun describe(status: Int) = when (status) {
            Proto.ST_NOT_PAIRED -> "Este celular não está pareado com o PC."
            Proto.ST_PAIRING_DISABLED -> "O PC não está aceitando novos pareamentos (veja a janela do Teclado Remoto)."
            Proto.ST_BUSY -> "O PC já está atendendo outro pedido de pareamento."
            Proto.ST_BAD_VERSION -> "Versões diferentes: atualize o app e o programa do PC."
            Proto.ST_REJECTED -> "O pareamento foi recusado no PC."
            Proto.ST_TIMEOUT -> "Ninguém confirmou no PC a tempo."
            else -> "O PC recusou a conexão (código $status)."
        }
    }
}
