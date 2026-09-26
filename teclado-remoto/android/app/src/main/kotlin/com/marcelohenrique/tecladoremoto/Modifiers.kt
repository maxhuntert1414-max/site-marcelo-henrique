package com.marcelohenrique.tecladoremoto

import com.marcelohenrique.tecladoremoto.core.Mods
import com.marcelohenrique.tecladoremoto.core.Vk

/** Ctrl/Shift/Alt/Win da barra: um toque = só a próxima tecla; dois toques = travado. */
class Modifiers(private val onChange: () -> Unit) {
    enum class State { OFF, ONCE, LOCKED }

    private val states = linkedMapOf(Mods.CTRL to State.OFF, Mods.SHIFT to State.OFF, Mods.ALT to State.OFF, Mods.WIN to State.OFF)

    fun state(mod: Int) = states[mod] ?: State.OFF

    val active: Int
        get() = states.entries.filter { it.value != State.OFF }.fold(0) { acc, e -> acc or e.key }

    fun tap(mod: Int) {
        states[mod] = when (state(mod)) {
            State.OFF -> State.ONCE
            State.ONCE -> State.LOCKED
            State.LOCKED -> State.OFF
        }
        onChange()
    }

    fun toggleLock(mod: Int) {
        states[mod] = if (state(mod) == State.LOCKED) State.OFF else State.LOCKED
        onChange()
    }

    /** Depois de usados numa tecla, os modificadores de um toque só se desligam. */
    fun consumeOnce() {
        var changed = false
        for (e in states.entries) {
            if (e.value == State.ONCE) {
                e.setValue(State.OFF)
                changed = true
            }
        }
        if (changed) onChange()
    }

    fun clear() {
        for (e in states.entries) e.setValue(State.OFF)
        onChange()
    }

    companion object {
        fun virtualKeys(mods: Int): List<Int> = buildList {
            if (mods and Mods.CTRL != 0) add(Vk.CONTROL)
            if (mods and Mods.SHIFT != 0) add(Vk.SHIFT)
            if (mods and Mods.ALT != 0) add(Vk.MENU)
            if (mods and Mods.WIN != 0) add(Vk.LWIN)
        }
    }
}
