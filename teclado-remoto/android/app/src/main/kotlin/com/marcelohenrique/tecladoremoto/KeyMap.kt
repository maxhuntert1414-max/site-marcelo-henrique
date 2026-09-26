package com.marcelohenrique.tecladoremoto

import android.view.KeyEvent
import com.marcelohenrique.tecladoremoto.core.Mods
import com.marcelohenrique.tecladoremoto.core.Vk

/** Teclas do Android (teclado físico/virtual) para virtual-keys do Windows. */
object KeyMap {
    fun special(keyCode: Int): Int? = when (keyCode) {
        KeyEvent.KEYCODE_FORWARD_DEL -> Vk.DELETE
        KeyEvent.KEYCODE_TAB -> Vk.TAB
        KeyEvent.KEYCODE_ESCAPE -> Vk.ESCAPE
        KeyEvent.KEYCODE_DPAD_UP -> Vk.UP
        KeyEvent.KEYCODE_DPAD_DOWN -> Vk.DOWN
        KeyEvent.KEYCODE_DPAD_LEFT -> Vk.LEFT
        KeyEvent.KEYCODE_DPAD_RIGHT -> Vk.RIGHT
        KeyEvent.KEYCODE_MOVE_HOME -> Vk.HOME
        KeyEvent.KEYCODE_MOVE_END -> Vk.END
        KeyEvent.KEYCODE_PAGE_UP -> Vk.PRIOR
        KeyEvent.KEYCODE_PAGE_DOWN -> Vk.NEXT
        KeyEvent.KEYCODE_INSERT -> Vk.INSERT
        in KeyEvent.KEYCODE_F1..KeyEvent.KEYCODE_F12 -> Vk.F1 + (keyCode - KeyEvent.KEYCODE_F1)
        KeyEvent.KEYCODE_SYSRQ -> Vk.SNAPSHOT
        KeyEvent.KEYCODE_BREAK -> Vk.PAUSE
        KeyEvent.KEYCODE_CAPS_LOCK -> Vk.CAPITAL
        KeyEvent.KEYCODE_SCROLL_LOCK -> Vk.SCROLL
        KeyEvent.KEYCODE_NUM_LOCK -> Vk.NUMLOCK
        KeyEvent.KEYCODE_MENU -> Vk.APPS
        KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE -> Vk.MEDIA_PLAY_PAUSE
        KeyEvent.KEYCODE_MEDIA_NEXT -> Vk.MEDIA_NEXT
        KeyEvent.KEYCODE_MEDIA_PREVIOUS -> Vk.MEDIA_PREV
        KeyEvent.KEYCODE_MEDIA_STOP -> Vk.MEDIA_STOP
        KeyEvent.KEYCODE_VOLUME_MUTE -> Vk.VOLUME_MUTE
        else -> null
    }

    fun letterOrDigit(keyCode: Int): Int? = when (keyCode) {
        in KeyEvent.KEYCODE_A..KeyEvent.KEYCODE_Z -> 'A'.code + (keyCode - KeyEvent.KEYCODE_A)
        in KeyEvent.KEYCODE_0..KeyEvent.KEYCODE_9 -> '0'.code + (keyCode - KeyEvent.KEYCODE_0)
        KeyEvent.KEYCODE_SPACE -> Vk.SPACE
        else -> null
    }

    fun modifiers(event: KeyEvent): Int {
        var mods = 0
        if (event.isCtrlPressed) mods = mods or Mods.CTRL
        if (event.isShiftPressed) mods = mods or Mods.SHIFT
        if (event.isAltPressed) mods = mods or Mods.ALT
        if (event.isMetaPressed) mods = mods or Mods.WIN
        return mods
    }
}
