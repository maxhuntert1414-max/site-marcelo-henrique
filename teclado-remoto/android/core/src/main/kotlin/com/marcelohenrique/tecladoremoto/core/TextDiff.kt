package com.marcelohenrique.tecladoremoto.core

import java.text.BreakIterator

/** Apagar [backspaces] caracteres e depois digitar [insert]. */
data class Edit(val backspaces: Int, val insert: String) {
    val isEmpty get() = backspaces == 0 && insert.isEmpty()
}

/**
 * Transforma "o campo era X, agora é Y" no menor Apagar+Digitar que leva o PC de X
 * para Y com o cursor no fim. Conta em caracteres visíveis (emoji, acento combinado
 * = um Backspace), porque é assim que o Backspace do Windows apaga.
 */
object TextDiff {
    fun diff(old: String, new: String): Edit {
        if (old == new) return Edit(0, "")
        val limit = minOf(old.length, new.length)
        var prefix = 0
        while (prefix < limit && old[prefix] == new[prefix]) prefix++
        if (prefix < old.length) {
            val it = BreakIterator.getCharacterInstance()
            it.setText(old)
            if (!it.isBoundary(prefix)) prefix = it.preceding(prefix)
        }
        return Edit(graphemes(old.substring(prefix)), new.substring(prefix))
    }

    fun graphemes(s: String): Int {
        if (s.isEmpty()) return 0
        val it = BreakIterator.getCharacterInstance()
        it.setText(s)
        var count = 0
        while (it.next() != BreakIterator.DONE) count++
        return count
    }
}
