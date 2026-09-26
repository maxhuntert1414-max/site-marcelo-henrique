package com.marcelohenrique.tecladoremoto.core

import java.math.BigInteger
import java.security.GeneralSecurityException
import java.security.KeyFactory
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.MessageDigest
import java.security.SecureRandom
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import java.security.spec.ECPoint
import java.security.spec.ECPublicKeySpec
import javax.crypto.Cipher
import javax.crypto.KeyAgreement
import javax.crypto.Mac
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec

/** ECDH P-256 + HKDF-SHA256 + AES-256-GCM, idêntico ao servidor do PC. */
object Crypto {
    val random = SecureRandom()

    fun randomBytes(n: Int) = ByteArray(n).also { random.nextBytes(it) }

    fun sha256(vararg parts: ByteArray): ByteArray {
        val md = MessageDigest.getInstance("SHA-256")
        for (p in parts) md.update(p)
        return md.digest()
    }

    private fun hmac(key: ByteArray, vararg parts: ByteArray): ByteArray {
        val mac = Mac.getInstance("HmacSHA256")
        mac.init(SecretKeySpec(key, "HmacSHA256"))
        for (p in parts) mac.update(p)
        return mac.doFinal()
    }

    fun hkdf(salt: ByteArray, ikm: ByteArray, info: ByteArray, length: Int): ByteArray {
        val prk = hmac(salt, ikm)
        val out = ByteArray(length)
        var block = ByteArray(0)
        var pos = 0
        var counter = 1
        while (pos < length) {
            block = hmac(prk, block, info, byteArrayOf(counter.toByte()))
            val n = minOf(block.size, length - pos)
            System.arraycopy(block, 0, out, pos, n)
            pos += n
            counter++
        }
        return out
    }

    fun commitment(serverPub: ByteArray, clientPub: ByteArray, serverNonce: ByteArray) =
        sha256("TRK1-commit".toByteArray(), serverPub, clientPub, serverNonce)

    /** Código de 6 dígitos que aparece no celular e no PC. */
    fun sasCode(clientPub: ByteArray, serverPub: ByteArray, nc: ByteArray, ns: ByteArray): Int {
        val h = sha256("TRK1-sas".toByteArray(), clientPub, serverPub, nc, ns)
        val v = ((h[0].toLong() and 0xFF) shl 24) or ((h[1].toLong() and 0xFF) shl 16) or
            ((h[2].toLong() and 0xFF) shl 8) or (h[3].toLong() and 0xFF)
        return (v % 1_000_000).toInt()
    }

    fun formatSas(code: Int) = "%03d %03d".format(code / 1000, code % 1000)

    fun pairKey(transcript: ByteArray, shared: ByteArray) = hkdf(transcript, shared, "TRK1-pair".toByteArray(), 32)

    /** (cliente→PC, PC→cliente) */
    fun sessionKeys(pairKey: ByteArray, shared: ByteArray, transcript: ByteArray): Pair<ByteArray, ByteArray> {
        val okm = hkdf(pairKey, shared, "TRK1-session".toByteArray() + transcript, 64)
        return okm.copyOfRange(0, 32) to okm.copyOfRange(32, 64)
    }

    fun hex(b: ByteArray): String {
        val chars = "0123456789abcdef"
        val sb = StringBuilder(b.size * 2)
        for (x in b) {
            sb.append(chars[(x.toInt() shr 4) and 15]).append(chars[x.toInt() and 15])
        }
        return sb.toString()
    }

    fun unhex(s: String): ByteArray? {
        if (s.length % 2 != 0) return null
        return ByteArray(s.length / 2) { i ->
            val hi = Character.digit(s[2 * i], 16)
            val lo = Character.digit(s[2 * i + 1], 16)
            if (hi < 0 || lo < 0) return null
            ((hi shl 4) or lo).toByte()
        }
    }
}

/** Par de chaves P-256 descartável (um por conexão). */
class Ephemeral {
    private val keyPair: KeyPair = KeyPairGenerator.getInstance("EC").run {
        initialize(ECGenParameterSpec("secp256r1"), Crypto.random)
        generateKeyPair()
    }

    val public: ByteArray = (keyPair.public as ECPublicKey).let { byteArrayOf(4) + fixed(it.w.affineX) + fixed(it.w.affineY) }

    /** Segredo compartilhado (coordenada X, 32 bytes). */
    fun agree(peer: ByteArray): ByteArray {
        if (peer.size != Proto.PUB_LEN || peer[0] != 4.toByte()) throw ProtocolException("Chave pública do PC inválida.")
        val params = (keyPair.public as ECPublicKey).params
        val point = ECPoint(BigInteger(1, peer.copyOfRange(1, 33)), BigInteger(1, peer.copyOfRange(33, 65)))
        val peerKey = try {
            KeyFactory.getInstance("EC").generatePublic(ECPublicKeySpec(point, params))
        } catch (e: GeneralSecurityException) {
            throw ProtocolException("Chave pública do PC inválida.")
        }
        val agreement = KeyAgreement.getInstance("ECDH")
        agreement.init(keyPair.private)
        agreement.doPhase(peerKey, true)
        return fixedBytes(agreement.generateSecret())
    }

    private companion object {
        fun fixed(v: BigInteger) = fixedBytes(v.toByteArray())

        /** Normaliza para exatamente 32 bytes (BigInteger pode sobrar/faltar zeros à esquerda). */
        fun fixedBytes(b: ByteArray): ByteArray = when {
            b.size == 32 -> b
            b.size > 32 -> b.copyOfRange(b.size - 32, b.size)
            else -> ByteArray(32 - b.size) + b
        }
    }
}

/** Cifra de um sentido da conexão; nonce = direção + contador (nunca se repete). */
class FrameCipher(key: ByteArray, private val direction: Int) {
    private val keySpec = SecretKeySpec(key, "AES")
    private val cipher = Cipher.getInstance("AES/GCM/NoPadding")
    private var counter = 0L

    private fun nextNonce(): ByteArray {
        val nonce = ByteArray(12)
        nonce[3] = direction.toByte()
        for (i in 0 until 8) nonce[4 + i] = (counter ushr (56 - 8 * i)).toByte()
        counter++
        return nonce
    }

    fun seal(plain: ByteArray): ByteArray {
        cipher.init(Cipher.ENCRYPT_MODE, keySpec, GCMParameterSpec(128, nextNonce()))
        return cipher.doFinal(plain)
    }

    fun open(frame: ByteArray): ByteArray? = try {
        cipher.init(Cipher.DECRYPT_MODE, keySpec, GCMParameterSpec(128, nextNonce()))
        cipher.doFinal(frame)
    } catch (e: GeneralSecurityException) {
        null
    }

    companion object {
        const val DIR_C2S = 1
        const val DIR_S2C = 2
    }
}
