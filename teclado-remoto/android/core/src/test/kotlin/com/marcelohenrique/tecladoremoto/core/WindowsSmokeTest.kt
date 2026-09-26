package com.marcelohenrique.tecladoremoto.core

import kotlin.test.Test

/**
 * Teste manual contra o .exe do Windows (máquina real ou Wine): pareia, espera alguém
 * clicar em "Permitir" no PC e digita um texto na janela em foco (ex.: Bloco de Notas).
 * Rode com TRK_SMOKE_HOST=ip e TRK_SMOKE_PORT=47800; sem as variáveis não faz nada.
 */
class WindowsSmokeTest {
    @Test
    fun pairAndType() {
        val host = System.getenv("TRK_SMOKE_HOST")?.takeIf { it.isNotBlank() } ?: return
        val port = System.getenv("TRK_SMOKE_PORT")?.toIntOrNull() ?: Proto.TCP_PORT
        val device = DeviceIdentity(Crypto.randomBytes(16), "Teste de fumaça")
        val paired = Session.pair(host, port, device, onCode = { println("CODIGO $it") })
        println("PAREADO com ${paired.session.serverName}")
        // Tempo para a janela de destino voltar ao foco depois do aviso de pareamento.
        Thread.sleep(3000)
        val s = paired.session
        s.send(Proto.type(0, "Olá, ação! Acentuação: çãõéü\n"))
        s.send(Proto.type(0, "Linha 2 com erro"))
        s.send(Proto.type(4, "acerto."))
        s.send(Proto.key(Proto.KEY_TAP, 0, Vk.RETURN))
        s.send(Proto.type(0, "Símbolos: @#\$%&*()[]{}<>/\\|~^`´"))
        s.send(Proto.ping(1))
        while (true) {
            val msg = s.receive()
            if ((msg[0].toInt() and 0xFF) == Proto.MSG_PONG && Proto.readLong(msg, 1) == 1L) break
        }
        s.send(Proto.bye())
        s.close()
        println("FIM")
    }
}
