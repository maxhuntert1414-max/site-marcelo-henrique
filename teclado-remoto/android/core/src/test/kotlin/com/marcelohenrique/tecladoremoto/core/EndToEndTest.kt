package com.marcelohenrique.tecladoremoto.core

import java.net.InetAddress
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

/**
 * Conversa de verdade com o servidor do PC rodando em modo headless
 * (tools/e2e.sh sobe o servidor e define TRK_E2E_PORT). Sem a variável, não faz nada.
 */
class EndToEndTest {
    private val port = System.getenv("TRK_E2E_PORT")?.toIntOrNull()
    private val discoveryPort = System.getenv("TRK_E2E_DISCOVERY_PORT")?.toIntOrNull() ?: Proto.DISCOVERY_PORT

    @Test
    fun pairTypeAndReconnect() {
        val port = port ?: return
        val found = mutableListOf<FoundServer>()
        Discovery.scan(timeoutMs = 800, targets = listOf(InetAddress.getLoopbackAddress()), port = discoveryPort) { found += it }
        assertEquals(1, found.size, "descoberta deve achar o servidor")
        assertEquals(port, found[0].port)

        val device = DeviceIdentity(Crypto.randomBytes(16), "Celular de teste")
        var code = ""
        val paired = Session.pair("127.0.0.1", port, device, onCode = { code = it })
        assertTrue(Regex("\\d{3} \\d{3}").matches(code), "código de pareamento: $code")
        assertContentEquals(found[0].id, paired.session.serverId)

        val session = paired.session
        session.send(Proto.type(0, "Olá, ação! 😀\n"))
        session.send(Proto.key(Proto.KEY_TAP, Mods.CTRL or Mods.SHIFT, Vk.letter('t')))
        session.send(Proto.char(Mods.CTRL, 'v'.code))
        session.send(Proto.clipSet("do celular"))
        session.send(Proto.clipGet())
        val clip = receiveType(session, Proto.MSG_CLIP_DATA)
        assertEquals("do celular", Proto.text(clip, 1))
        session.send(Proto.key(Proto.KEY_DOWN, 0, Vk.MENU))
        session.send(Proto.mouseMove(-3, 4))
        awaitPong(session, 42)
        session.close()

        val again = Session.connect("127.0.0.1", port, device, paired.pairKey)
        again.send(Proto.type(2, "fim"))
        awaitPong(again, 7)
        again.send(Proto.bye())
        again.close()

        val stranger = DeviceIdentity(Crypto.randomBytes(16), "Intruso")
        val refused = assertFailsWith<RefusedException> { Session.connect("127.0.0.1", port, stranger, ByteArray(32)) }
        assertEquals(Proto.ST_NOT_PAIRED, refused.status)
        assertFailsWith<RefusedException> { Session.connect("127.0.0.1", port, device, ByteArray(32)) }
    }

    /** O PING enviado ao conectar também gera PONG; espera o nosso. */
    private fun awaitPong(session: Session, stamp: Long) {
        session.send(Proto.ping(stamp))
        while (Proto.readLong(receiveType(session, Proto.MSG_PONG), 1) != stamp) Unit
    }

    private fun receiveType(session: Session, type: Int): ByteArray {
        while (true) {
            val msg = session.receive()
            if ((msg[0].toInt() and 0xFF) == type) return msg
        }
    }
}
