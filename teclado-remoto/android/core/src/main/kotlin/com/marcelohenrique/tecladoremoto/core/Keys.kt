package com.marcelohenrique.tecladoremoto.core

/** Modificadores do protocolo. */
object Mods {
    const val CTRL = 1
    const val SHIFT = 2
    const val ALT = 4
    const val WIN = 8

    /** Modificadores que transformam uma letra em atalho (Shift sozinho não). */
    const val SHORTCUT = CTRL or ALT or WIN
}

/** Códigos virtual-key do Windows. */
object Vk {
    const val BACK = 0x08
    const val TAB = 0x09
    const val RETURN = 0x0D
    const val SHIFT = 0x10
    const val CONTROL = 0x11
    const val MENU = 0x12
    const val PAUSE = 0x13
    const val CAPITAL = 0x14
    const val ESCAPE = 0x1B
    const val SPACE = 0x20
    const val PRIOR = 0x21
    const val NEXT = 0x22
    const val END = 0x23
    const val HOME = 0x24
    const val LEFT = 0x25
    const val UP = 0x26
    const val RIGHT = 0x27
    const val DOWN = 0x28
    const val SNAPSHOT = 0x2C
    const val INSERT = 0x2D
    const val DELETE = 0x2E
    const val LWIN = 0x5B
    const val APPS = 0x5D
    const val F1 = 0x70
    const val NUMLOCK = 0x90
    const val SCROLL = 0x91
    const val BROWSER_BACK = 0xA6
    const val BROWSER_FORWARD = 0xA7
    const val BROWSER_REFRESH = 0xA8
    const val BROWSER_HOME = 0xAC
    const val VOLUME_MUTE = 0xAD
    const val VOLUME_DOWN = 0xAE
    const val VOLUME_UP = 0xAF
    const val MEDIA_NEXT = 0xB0
    const val MEDIA_PREV = 0xB1
    const val MEDIA_STOP = 0xB2
    const val MEDIA_PLAY_PAUSE = 0xB3
    const val OEM_PLUS = 0xBB
    const val OEM_MINUS = 0xBD
    const val OEM_PERIOD = 0xBE

    fun f(n: Int) = F1 + n - 1
    fun letter(c: Char) = c.uppercaseChar().code

    /** Teclas que mexem no cursor/foco do PC: depois delas o campo do celular recomeça vazio. */
    fun movesCaret(vk: Int) = vk in PRIOR..DOWN || vk == TAB || vk == RETURN || vk == ESCAPE ||
        vk == HOME || vk == END || vk == DELETE || vk == INSERT || vk in F1..F1 + 23
}
