package com.marcelohenrique.tecladoremoto.core

import java.io.BufferedInputStream
import java.io.DataInputStream
import java.io.IOException
import java.io.OutputStream
import java.net.InetSocketAddress
import java.net.Socket
import java.security.MessageDigest

/** Quem é este celular para o PC. */
class DeviceIdentity(val id: ByteArray, val name: String)

class PairResult(val session: Session, val pairKey: ByteArray)

/**
 * Conexão cifrada já autenticada com o PC. [send] pode ser chamado de uma thread
 * e [receive] de outra; cada sentido tem sua própria cifra.
 */
class Session private constructor(
    private val socket: Socket,
    private val input: DataInputStream,
    private val output: OutputStream,
    private val tx: FrameCipher,
    private val rx: FrameCipher,
    val serverId: ByteArray,
    val serverName: String,
    val serverElevated: Boolean,
) {
    @Volatile
    var closed = false
        private set

    fun send(plain: ByteArray) {
        synchronized(tx) {
            writeFrame(output, tx.seal(plain))
        }
    }

    /** Próxima mensagem do PC (bloqueia até chegar ou dar o tempo limite de leitura). */
    fun receive(): ByteArray {
        val frame = readFrame(input, Proto.MAX_PLAINTEXT + Proto.TAG_LEN)
        return rx.open(frame) ?: throw ProtocolException("Mensagem do PC não confere (chave diferente?).")
    }

    fun close() {
        closed = true
        try {
            socket.close()
        } catch (_: IOException) {
        }
    }

    companion object {
        const val CONNECT_TIMEOUT_MS = 2500
        const val HANDSHAKE_TIMEOUT_MS = 8000
        const val SESSION_READ_TIMEOUT_MS = 7000
        const val PAIR_DECISION_TIMEOUT_MS = 75_000

        private fun open(host: String, port: Int): Socket {
            val socket = Socket()
            try {
                socket.tcpNoDelay = true
                socket.keepAlive = true
                try {
                    // DSCP "Expedited Forwarding": roteadores com WMM priorizam como voz/vídeo.
                    socket.trafficClass = 0xB8
                } catch (_: Exception) {
                }
                socket.connect(InetSocketAddress(host, port), CONNECT_TIMEOUT_MS)
                socket.soTimeout = HANDSHAKE_TIMEOUT_MS
            } catch (e: IOException) {
                socket.close()
                throw e
            }
            return socket
        }

        private fun streams(socket: Socket) =
            DataInputStream(BufferedInputStream(socket.getInputStream(), 16 * 1024)) to socket.getOutputStream()

        /** Conecta a um PC já pareado. Com [expectedServerId], recusa outro PC que esteja no mesmo IP. */
        fun connect(
            host: String,
            port: Int,
            device: DeviceIdentity,
            pairKey: ByteArray,
            expectedServerId: ByteArray? = null,
        ): Session {
            val socket = open(host, port)
            try {
                val (input, output) = streams(socket)
                val eph = Ephemeral()
                val hello = Proto.clientHello(Proto.MODE_SESSION, device.id, eph.public, Crypto.randomBytes(32), device.name)
                writeFrame(output, hello)
                val server = ServerHello.parse(readFrame(input, 512))
                if (server.status != Proto.ST_OK) throw RefusedException(server.status)
                if (expectedServerId != null && !server.serverId.contentEquals(expectedServerId)) {
                    throw WrongServerException()
                }
                val shared = eph.agree(server.public)
                val transcript = Crypto.sha256(hello, server.raw)
                return finish(socket, input, output, server, Crypto.sessionKeys(pairKey, shared, transcript))
            } catch (e: IOException) {
                socket.close()
                throw e
            }
        }

        /**
         * Pareamento por comparação numérica. [onCode] recebe o código de 6 dígitos que o
         * usuário deve conferir no PC; a função só retorna quando o PC aceita ou recusa.
         */
        fun pair(host: String, port: Int, device: DeviceIdentity, onCode: (String) -> Unit, socketReady: (Socket) -> Unit = {}): PairResult {
            val socket = open(host, port)
            socketReady(socket)
            try {
                val (input, output) = streams(socket)
                val eph = Ephemeral()
                val hello = Proto.clientHello(Proto.MODE_PAIR, device.id, eph.public, ByteArray(32), device.name)
                writeFrame(output, hello)
                val server = ServerHello.parse(readFrame(input, 512))
                if (server.status != Proto.ST_OK) throw RefusedException(server.status)

                val nc = Crypto.randomBytes(32)
                writeFrame(output, nc)
                val ns = readFrame(input, 32)
                if (ns.size != 32) throw ProtocolException("Resposta de pareamento inválida.")
                val expected = Crypto.commitment(server.public, eph.public, ns)
                if (!MessageDigest.isEqual(expected, server.nonceOrCommit)) {
                    throw ProtocolException("O compromisso do PC não confere. Pareamento cancelado por segurança.")
                }
                onCode(Crypto.formatSas(Crypto.sasCode(eph.public, server.public, nc, ns)))

                socket.soTimeout = PAIR_DECISION_TIMEOUT_MS
                val decision = readFrame(input, 1)
                val status = decision[0].toInt() and 0xFF
                if (status != Proto.ST_OK) throw RefusedException(status)

                val shared = eph.agree(server.public)
                val transcript = Crypto.sha256(hello, server.raw, nc, ns)
                val pairKey = Crypto.pairKey(transcript, shared)
                val session = finish(socket, input, output, server, Crypto.sessionKeys(pairKey, shared, transcript))
                return PairResult(session, pairKey)
            } catch (e: IOException) {
                socket.close()
                throw e
            }
        }

        private fun finish(
            socket: Socket,
            input: DataInputStream,
            output: OutputStream,
            server: ServerHello,
            keys: Pair<ByteArray, ByteArray>,
        ): Session {
            val tx = FrameCipher(keys.first, FrameCipher.DIR_C2S)
            val rx = FrameCipher(keys.second, FrameCipher.DIR_S2C)
            socket.soTimeout = HANDSHAKE_TIMEOUT_MS
            val welcome = rx.open(readFrame(input, 512))
                ?: throw RefusedException(Proto.ST_NOT_PAIRED)
            if (welcome.isEmpty() || (welcome[0].toInt() and 0xFF) != Proto.MSG_WELCOME || welcome.size < 2) {
                throw ProtocolException("Boas-vindas do PC inválidas.")
            }
            val elevated = (welcome[1].toInt() and Proto.WELCOME_FLAG_ELEVATED) != 0
            socket.soTimeout = SESSION_READ_TIMEOUT_MS
            val session = Session(socket, input, output, tx, rx, server.serverId, server.name, elevated)
            // A primeira mensagem cifrada prova ao PC que este celular tem a chave.
            session.send(Proto.ping(System.nanoTime()))
            return session
        }

        fun writeFrame(output: OutputStream, payload: ByteArray) {
            val frame = ByteArray(4 + payload.size)
            val n = payload.size
            frame[0] = (n ushr 24).toByte()
            frame[1] = (n ushr 16).toByte()
            frame[2] = (n ushr 8).toByte()
            frame[3] = n.toByte()
            System.arraycopy(payload, 0, frame, 4, n)
            // Uma escrita só = um pacote só.
            output.write(frame)
            output.flush()
        }

        fun readFrame(input: DataInputStream, max: Int): ByteArray {
            val n = input.readInt()
            if (n <= 0 || n > max) throw ProtocolException("Mensagem com tamanho inválido.")
            return ByteArray(n).also { input.readFully(it) }
        }
    }
}
