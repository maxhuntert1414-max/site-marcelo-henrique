package com.marcelohenrique.tecladoremoto.core

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class CryptoTest {
    @Test
    fun hkdfMatchesRfc5869Case1() {
        val okm = Crypto.hkdf(
            Crypto.unhex("000102030405060708090a0b0c")!!,
            ByteArray(22) { 0x0b },
            Crypto.unhex("f0f1f2f3f4f5f6f7f8f9")!!,
            42,
        )
        assertEquals(
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865",
            Crypto.hex(okm),
        )
    }

    @Test
    fun ecdhAgreesBothWays() {
        val a = Ephemeral()
        val b = Ephemeral()
        assertEquals(65, a.public.size)
        assertContentEquals(a.agree(b.public), b.agree(a.public))
    }

    @Test
    fun framesOpenOnlyInOrder() {
        val key = ByteArray(32) { 9 }
        val tx = FrameCipher(key, FrameCipher.DIR_C2S)
        val rx = FrameCipher(key, FrameCipher.DIR_C2S)
        val f1 = tx.seal("um".toByteArray())
        val f2 = tx.seal("dois".toByteArray())
        assertEquals(2 + 16, f1.size)
        assertContentEquals("um".toByteArray(), rx.open(f1))
        assertContentEquals("dois".toByteArray(), rx.open(f2))
        assertNull(FrameCipher(key, FrameCipher.DIR_S2C).open(f1))
        val replay = FrameCipher(key, FrameCipher.DIR_C2S)
        replay.open(f1)
        assertNull(replay.open(f1))
    }

    @Test
    fun sasAndHex() {
        assertTrue(Crypto.sasCode(ByteArray(65) { 1 }, ByteArray(65) { 2 }, ByteArray(32) { 3 }, ByteArray(32) { 4 }) < 1_000_000)
        assertEquals("000 042", Crypto.formatSas(42))
        assertEquals("00ab10", Crypto.hex(byteArrayOf(0, 0xab.toByte(), 0x10)))
        assertContentEquals(byteArrayOf(0, 0xab.toByte(), 0x10), Crypto.unhex("00AB10"))
        assertNull(Crypto.unhex("0g"))
        assertNull(Crypto.unhex("abc"))
    }
}

class ProtocolTest {
    @Test
    fun helloLayout() {
        val hello = Proto.clientHello(Proto.MODE_PAIR, ByteArray(16) { 7 }, ByteArray(65) { 4 }, ByteArray(32), "Moto G")
        assertEquals(120 + 6, hello.size)
        assertEquals('T'.code.toByte(), hello[0])
        assertEquals(1.toByte(), hello[4])
        assertEquals(2.toByte(), hello[5])
        assertEquals(6.toByte(), hello[119])
    }

    @Test
    fun messagesMatchServerLayout() {
        assertContentEquals(byteArrayOf(0x01, 0, 3) + "olá\n".toByteArray(), Proto.type(3, "olá\n"))
        assertContentEquals(byteArrayOf(0x02, 0, 1, 0, 0x43), Proto.key(Proto.KEY_TAP, Mods.CTRL, 0x43))
        assertContentEquals(byteArrayOf(0x03, 1, 0, 0, 0, 0x63), Proto.char(Mods.CTRL, 'c'.code))
        assertContentEquals(byteArrayOf(0x20, -1, -2, 0, 5), Proto.mouseMove(-2, 5))
        val ping = Proto.ping(0x0102030405060708)
        assertEquals(0x0102030405060708, Proto.readLong(ping, 1))
    }

    @Test
    fun utf8PrefixNeverSplitsCharacters() {
        assertContentEquals("ab".toByteArray(), Proto.utf8Prefix("abç", 3))
        assertContentEquals("abç".toByteArray(), Proto.utf8Prefix("abç", 4))
        assertEquals(0, Proto.utf8Prefix("😀", 3).size)
    }

    @Test
    fun parsesDiscoveryReply() {
        val id = ByteArray(16) { it.toByte() }
        val reply = "TRK1!".toByteArray() + byteArrayOf(1, 2, 3, 4, 1) + id + byteArrayOf(0xBA.toByte(), 0xB8.toByte(), 2) + "PC".toByteArray()
        val found = assertNotNull(Discovery.parseReply(reply, reply.size, byteArrayOf(1, 2, 3, 4), "192.168.0.10"))
        assertEquals(47800, found.port)
        assertEquals("PC", found.name)
        assertContentEquals(id, found.id)
        assertNull(Discovery.parseReply(reply, reply.size, byteArrayOf(9, 9, 9, 9), "x"))
    }
}

class TextDiffTest {
    @Test
    fun typingAppends() = assertEquals(Edit(0, "o"), TextDiff.diff("ol", "olo"))

    @Test
    fun backspaceRemoves() = assertEquals(Edit(1, ""), TextDiff.diff("olá", "ol"))

    @Test
    fun autocorrectRewritesOnlyTheTail() = assertEquals(Edit(3, "he "), TextDiff.diff("teh ", "the "))

    @Test
    fun suggestionReplacesWord() = assertEquals(Edit(1, "ção "), TextDiff.diff("informac", "informação "))

    @Test
    fun emojiCountsAsOneBackspace() {
        assertEquals(Edit(1, ""), TextDiff.diff("oi😀", "oi"))
        assertEquals(Edit(1, "🎉"), TextDiff.diff("oi😀", "oi🎉"))
        assertEquals(Edit(1, ""), TextDiff.diff("a👍🏽", "a"))
    }

    @Test
    fun combiningAccentIsNotSplit() {
        // "é" decomposto (e + acento): trocar por "e" apaga o caractere inteiro e redigita.
        assertEquals(Edit(1, "e"), TextDiff.diff("é", "e"))
    }

    @Test
    fun unchanged() = assertTrue(TextDiff.diff("abc", "abc").isEmpty)
}
