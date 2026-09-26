package com.marcelohenrique.tecladoremoto

import android.os.Handler
import android.os.Looper
import android.os.Process
import com.marcelohenrique.tecladoremoto.core.Crypto
import com.marcelohenrique.tecladoremoto.core.Discovery
import com.marcelohenrique.tecladoremoto.core.Proto
import com.marcelohenrique.tecladoremoto.core.RefusedException
import com.marcelohenrique.tecladoremoto.core.Session
import com.marcelohenrique.tecladoremoto.core.WrongServerException
import java.io.IOException
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit

/**
 * Mantém a conexão com um PC: reconecta sozinho, acha o PC de novo se o IP mudar,
 * manda as mensagens por uma única thread (ordem garantida) e mede a latência.
 * Os callbacks chegam na thread principal.
 */
class RemoteClient(private val store: ServerStore, private val listener: Listener) {
    enum class State { CONNECTING, CONNECTED, OFFLINE, NEEDS_PAIRING }

    interface Listener {
        fun onState(state: State, detail: String)
        fun onLatency(ms: Int)
        fun onClipboard(text: String)
        fun onNotice(text: String)
    }

    private val main = Handler(Looper.getMainLooper())
    private val queue = LinkedBlockingQueue<ByteArray>(4096)

    @Volatile
    private var generation = 0

    @Volatile
    private var session: Session? = null

    @Volatile
    var server: SavedServer? = null
        private set

    val connected get() = session != null

    fun start(target: SavedServer, established: Session? = null) {
        stop()
        server = target
        val gen = generation
        Thread({ loop(target, gen, established) }, "conexao").start()
    }

    fun stop() {
        generation++
        server = null
        val old = session
        session = null
        queue.clear()
        if (old != null) {
            Thread({
                try {
                    old.send(Proto.bye())
                } catch (_: IOException) {
                }
                old.close()
            }, "encerrar").start()
        }
    }

    /** `false` se não há conexão agora (a mensagem é descartada). */
    fun send(message: ByteArray): Boolean = session != null && queue.offer(message)

    private fun post(gen: Int, block: () -> Unit) {
        main.post { if (gen == generation) block() }
    }

    private fun loop(target: SavedServer, gen: Int, established: Session?) {
        var pending = established
        var current = target
        var failures = 0
        while (gen == generation) {
            if (failures == 0) post(gen) { listener.onState(State.CONNECTING, current.name) }
            val s = pending ?: try {
                Session.connect(current.host, current.port, store.identity, current.key, Crypto.unhex(current.id))
            } catch (e: RefusedException) {
                if (e.status == Proto.ST_NOT_PAIRED) {
                    // Pode ser outro PC que ficou com o IP antigo; só pede novo pareamento
                    // se o nosso PC não estiver em outro endereço.
                    val moved = rediscover(current)
                    if (moved == null) {
                        post(gen) { listener.onState(State.NEEDS_PAIRING, e.message.orEmpty()) }
                        return
                    }
                    current = moved
                    store.save(moved)
                    continue
                }
                null
            } catch (_: WrongServerException) {
                null
            } catch (_: IOException) {
                null
            }
            pending = null
            if (s == null) {
                failures++
                // O PC pode ter trocado de IP (DHCP): procura pelo mesmo id na rede.
                rediscover(current)?.let { found ->
                    current = found
                    store.save(found)
                }
                if (gen != generation) return
                post(gen) { listener.onState(State.OFFLINE, "Procurando o PC…") }
                sleep(minOf(250L * failures, 2000L))
                continue
            }
            if (gen != generation) {
                s.close()
                return
            }
            failures = 0
            queue.clear()
            session = s
            val name = s.serverName
            if (name.isNotEmpty() && name != current.name) {
                current = SavedServer(current.id, name, current.host, current.port, current.key)
                store.save(current)
            }
            post(gen) { listener.onState(State.CONNECTED, name) }
            runSession(s, gen)
            if (session === s) session = null
            if (gen != generation) return
            post(gen) { listener.onState(State.OFFLINE, "Conexão perdida. Reconectando…") }
            sleep(200)
        }
    }

    private fun rediscover(target: SavedServer): SavedServer? {
        var found: SavedServer? = null
        try {
            Discovery.scan(timeoutMs = 1200) {
                if (it.idHex == target.id && found == null) {
                    found = SavedServer(target.id, it.name, it.host, it.port, target.key)
                }
            }
        } catch (_: IOException) {
        }
        return found?.takeIf { it.host != target.host || it.port != target.port }
    }

    private fun runSession(s: Session, gen: Int) {
        val writer = Thread({
            try {
                Process.setThreadPriority(Process.THREAD_PRIORITY_URGENT_DISPLAY)
            } catch (_: Exception) {
            }
            try {
                while (gen == generation && !s.closed) {
                    s.send(queue.poll(PING_INTERVAL_MS, TimeUnit.MILLISECONDS) ?: Proto.ping(System.nanoTime()))
                }
            } catch (_: Exception) {
            }
            s.close()
        }, "envio")
        writer.start()
        try {
            while (gen == generation) {
                val msg = s.receive()
                when (msg[0].toInt() and 0xFF) {
                    Proto.MSG_PONG -> if (msg.size == 9) {
                        val ms = ((System.nanoTime() - Proto.readLong(msg, 1)) / 1_000_000).toInt()
                        post(gen) { listener.onLatency(ms) }
                    }
                    Proto.MSG_CLIP_DATA -> {
                        val text = Proto.text(msg, 1)
                        post(gen) { listener.onClipboard(text) }
                    }
                    Proto.MSG_NOTICE -> if (msg.size >= 2) {
                        val text = Proto.text(msg, 2)
                        post(gen) { listener.onNotice(text) }
                    }
                }
            }
        } catch (_: IOException) {
        } finally {
            s.close()
            writer.interrupt()
        }
    }

    private fun sleep(ms: Long) {
        try {
            Thread.sleep(ms)
        } catch (_: InterruptedException) {
        }
    }

    private companion object {
        const val PING_INTERVAL_MS = 2000L
    }
}
