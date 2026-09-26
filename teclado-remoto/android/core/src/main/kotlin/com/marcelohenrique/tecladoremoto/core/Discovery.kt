package com.marcelohenrique.tecladoremoto.core

import java.io.IOException
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.Inet4Address
import java.net.InetAddress
import java.net.NetworkInterface
import java.net.SocketTimeoutException

class FoundServer(val id: ByteArray, val name: String, val host: String, val port: Int) {
    val idHex: String get() = Crypto.hex(id)
}

/** Acha PCs com o Teclado Remoto aberto na rede local (broadcast UDP). */
object Discovery {
    private val PROBE = "TRK1?".toByteArray()
    private val REPLY = "TRK1!".toByteArray()

    fun broadcastTargets(): List<InetAddress> {
        val targets = linkedSetOf<InetAddress>(InetAddress.getByName("255.255.255.255"))
        try {
            for (nif in NetworkInterface.getNetworkInterfaces() ?: return targets.toList()) {
                if (!nif.isUp || nif.isLoopback) continue
                for (address in nif.interfaceAddresses) {
                    if (address.address is Inet4Address) address.broadcast?.let(targets::add)
                }
            }
        } catch (_: Exception) {
        }
        return targets.toList()
    }

    /**
     * Manda a sonda 3 vezes (a rede pode perder pacotes) e chama [onFound] para cada PC
     * que responder dentro de [timeoutMs]. Bloqueia; rode fora da thread principal.
     */
    fun scan(
        timeoutMs: Long = 1500,
        targets: List<InetAddress> = broadcastTargets(),
        port: Int = Proto.DISCOVERY_PORT,
        onFound: (FoundServer) -> Unit,
    ) {
        DatagramSocket().use { socket ->
            socket.broadcast = true
            socket.soTimeout = 100
            val probeId = Crypto.randomBytes(4)
            val probe = PROBE + probeId
            val seen = HashSet<String>()
            val buffer = ByteArray(512)
            val start = System.nanoTime()
            var sent = 0
            while (true) {
                val elapsed = (System.nanoTime() - start) / 1_000_000
                if (elapsed >= timeoutMs) break
                if (sent < 3 && elapsed >= sent * 350L) {
                    for (target in targets) {
                        try {
                            socket.send(DatagramPacket(probe, probe.size, target, port))
                        } catch (_: IOException) {
                        }
                    }
                    sent++
                }
                val packet = DatagramPacket(buffer, buffer.size)
                try {
                    socket.receive(packet)
                } catch (_: SocketTimeoutException) {
                    continue
                }
                val found = parseReply(packet.data, packet.length, probeId, packet.address.hostAddress ?: continue)
                if (found != null && seen.add(found.idHex + "@" + found.host)) onFound(found)
            }
        }
    }

    fun parseReply(b: ByteArray, length: Int, probeId: ByteArray, host: String): FoundServer? {
        if (length < 29) return null
        for (i in REPLY.indices) if (b[i] != REPLY[i]) return null
        for (i in 0 until 4) if (b[5 + i] != probeId[i]) return null
        if (b[9].toInt() != Proto.VERSION) return null
        val nameLen = b[28].toInt() and 0xFF
        if (length != 29 + nameLen) return null
        val port = ((b[26].toInt() and 0xFF) shl 8) or (b[27].toInt() and 0xFF)
        return FoundServer(b.copyOfRange(10, 26), String(b, 29, nameLen, Charsets.UTF_8), host, port)
    }
}
