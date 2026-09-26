package com.marcelohenrique.tecladoremoto

import android.content.Context
import android.text.Editable
import android.text.InputType
import android.text.TextWatcher
import android.util.AttributeSet
import android.view.KeyEvent
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputConnectionWrapper
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import com.marcelohenrique.tecladoremoto.core.Mods
import com.marcelohenrique.tecladoremoto.core.TextDiff
import com.marcelohenrique.tecladoremoto.core.Vk

/**
 * Campo que captura o teclado do celular (Gboard, Samsung, SwiftKey...) e espelha no PC.
 *
 * Em vez de tentar adivinhar cada tecla, compara o texto do campo antes/depois de cada
 * mudança e manda só a diferença (Backspaces + texto). Assim funciona com sugestões,
 * autocorreção, deslizar para digitar e ditado por voz. O cursor fica sempre no fim, então
 * o cursor do PC acompanha.
 */
class CaptureEditText(context: Context, attrs: AttributeSet?) : EditText(context, attrs) {
    enum class Mode { LIVE, DIRECT, COMPOSE }

    interface Sink {
        /** Modificadores fixados na barra (Ctrl/Shift/Alt/Win). */
        fun stickyModifiers(): Int

        fun onEdit(backspaces: Int, text: String)
        fun onKey(vk: Int, mods: Int)
        fun onChar(codepoint: Int, mods: Int)
        fun onSubmit(text: String, pressEnter: Boolean)
    }

    var sink: Sink? = null

    var mode = Mode.LIVE
        set(value) {
            field = value
            applyMode()
        }

    /** O que o PC já recebeu deste campo. */
    private var mirror = ""
    private var muted = false
    private var batchDepth = 0
    private var pendingFlush = false
    private val safetyFlush = Runnable { flush() }

    /** O construtor de TextView chama métodos sobrescritos antes das propriedades existirem. */
    private var ready = false

    init {
        isSaveEnabled = false
        addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) = Unit
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) = Unit
            override fun afterTextChanged(s: Editable?) = onChanged()
        })
        ready = true
        applyMode()
    }

    private fun applyMode() {
        muted = true
        var options = EditorInfo.IME_FLAG_NO_EXTRACT_UI or EditorInfo.IME_FLAG_NO_FULLSCREEN
        options = options or if (mode == Mode.COMPOSE) {
            EditorInfo.IME_ACTION_SEND
        } else {
            EditorInfo.IME_FLAG_NO_ENTER_ACTION
        }
        if (mode == Mode.DIRECT && android.os.Build.VERSION.SDK_INT >= 26) {
            options = options or EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING
        }
        imeOptions = options
        inputType = when (mode) {
            // Sem sugestões nem autocorreção: cada tecla vai exatamente como foi digitada.
            Mode.DIRECT -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD or
                InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
            Mode.LIVE -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE or
                InputType.TYPE_TEXT_FLAG_AUTO_CORRECT
            Mode.COMPOSE -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_AUTO_CORRECT or
                InputType.TYPE_TEXT_FLAG_CAP_SENTENCES
        }
        maxLines = if (mode == Mode.LIVE) 3 else 1
        muted = false
        resetBuffer()
        (context.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager).restartInput(this)
    }

    /** Esquece o conteúdo do campo sem mexer no PC (o cursor do PC mudou de lugar). */
    fun resetBuffer() {
        removeCallbacks(safetyFlush)
        pendingFlush = false
        val editable = text
        val composing = editable != null && BaseInputConnection.getComposingSpanStart(editable) >= 0
        muted = true
        editable?.clear()
        muted = false
        mirror = ""
        if (composing) {
            // O teclado ainda achava que havia uma palavra sendo composta: recomeça do zero.
            (context.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager).restartInput(this)
        }
    }

    private fun onChanged() {
        if (muted) return
        if (batchDepth > 0) {
            // O teclado está no meio de uma edição composta: manda tudo junto no fim.
            pendingFlush = true
            removeCallbacks(safetyFlush)
            postDelayed(safetyFlush, 120)
            return
        }
        flush()
    }

    private fun flush() {
        removeCallbacks(safetyFlush)
        pendingFlush = false
        val sink = sink ?: return
        if (mode == Mode.COMPOSE) return
        val current = text?.toString().orEmpty()
        if (current == mirror) return
        val edit = TextDiff.diff(mirror, current)
        val sticky = sink.stickyModifiers()
        mirror = current
        if (sticky and Mods.SHORTCUT != 0) {
            // Com Ctrl/Alt/Win ativos, o que foi digitado vira atalho (Ctrl+C, Alt+F4...).
            repeat(edit.backspaces) { sink.onKey(Vk.BACK, 0) }
            var i = 0
            while (i < edit.insert.length) {
                val cp = edit.insert.codePointAt(i)
                if (cp == '\n'.code) sink.onKey(Vk.RETURN, 0) else sink.onChar(cp, 0)
                i += Character.charCount(cp)
            }
            post { resetBuffer() }
            return
        }
        sink.onEdit(edit.backspaces, edit.insert)
        if (edit.insert.contains('\n') || current.length > MAX_BUFFER) post { resetBuffer() }
    }

    fun submit(pressEnter: Boolean) {
        val current = text?.toString().orEmpty()
        sink?.onSubmit(current, pressEnter)
        resetBuffer()
    }

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection? {
        val target = super.onCreateInputConnection(outAttrs) ?: return null
        return Connection(target)
    }

    private inner class Connection(target: InputConnection) : InputConnectionWrapper(target, true) {
        override fun beginBatchEdit(): Boolean {
            batchDepth++
            return super.beginBatchEdit()
        }

        override fun endBatchEdit(): Boolean {
            val result = super.endBatchEdit()
            if (batchDepth > 0) batchDepth--
            if (batchDepth == 0 && pendingFlush) flush()
            return result
        }

        override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
            val caret = selectionStart.coerceAtLeast(0)
            val beyond = beforeLength - caret
            if (beyond <= 0) return super.deleteSurroundingText(beforeLength, afterLength)
            // Pediram para apagar mais do que existe no campo: o excedente vai direto ao PC.
            val result = super.deleteSurroundingText(caret, afterLength)
            flush()
            repeat(beyond) { sink?.onKey(Vk.BACK, 0) }
            return result
        }

        override fun setSelection(start: Int, end: Int): Boolean {
            // O teclado quer mover o cursor (ex.: deslizar na barra de espaço): move no PC com
            // as setas e recomeça o campo, porque o cursor do PC saiu do fim do texto.
            if (mode == Mode.COMPOSE || start != end) return super.setSelection(start, end)
            val caret = selectionEnd.coerceAtLeast(0)
            val current = text?.toString().orEmpty()
            val target = start.coerceIn(0, current.length)
            if (target == caret) return true
            flush()
            val steps = TextDiff.graphemes(current.substring(minOf(target, caret), maxOf(target, caret)))
            repeat(steps) { sink?.onKey(if (target < caret) Vk.LEFT else Vk.RIGHT, 0) }
            post { resetBuffer() }
            return true
        }
    }

    override fun onSelectionChanged(selStart: Int, selEnd: Int) {
        super.onSelectionChanged(selStart, selEnd)
        if (!ready || mode == Mode.COMPOSE || batchDepth > 0 || selStart != selEnd) return
        // Toque no meio do campo: o cursor volta para o fim, que é onde o cursor do PC está.
        // (Mesmo sem isso o espelho continuaria certo, só gastaria mais Backspaces.)
        val length = text?.length ?: 0
        if (selStart != length) setSelection(length)
    }

    override fun onEditorAction(actionCode: Int) {
        if (mode == Mode.COMPOSE) submit(pressEnter = true) else pressEnter(0)
    }

    private fun pressEnter(mods: Int) {
        flush()
        sink?.onKey(Vk.RETURN, mods)
        resetBuffer()
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean = handleKey(event) || super.onKeyDown(keyCode, event)

    override fun onKeyUp(keyCode: Int, event: KeyEvent): Boolean =
        (KeyMap.special(keyCode) != null || keyCode == KeyEvent.KEYCODE_ENTER) || super.onKeyUp(keyCode, event)

    /** Teclas "de verdade" (teclado físico ou teclas especiais do teclado virtual). */
    private fun handleKey(event: KeyEvent): Boolean {
        val sink = sink ?: return false
        val code = event.keyCode
        val meta = KeyMap.modifiers(event)
        val empty = text.isNullOrEmpty()
        when {
            code == KeyEvent.KEYCODE_ENTER || code == KeyEvent.KEYCODE_NUMPAD_ENTER -> {
                if (mode == Mode.COMPOSE) submit(pressEnter = true) else pressEnter(meta)
                return true
            }
            code == KeyEvent.KEYCODE_DEL -> {
                if (!empty && meta == 0 && mode != Mode.COMPOSE) return false
                if (!empty && mode == Mode.COMPOSE) return false
                flush()
                sink.onKey(Vk.BACK, meta)
                if (meta != 0) resetBuffer()
                return true
            }
        }
        val special = KeyMap.special(code)
        if (special != null) {
            flush()
            sink.onKey(special, meta)
            if (Vk.movesCaret(special)) resetBuffer()
            return true
        }
        if (meta and Mods.SHORTCUT != 0) {
            val vk = KeyMap.letterOrDigit(code)
            if (vk != null) {
                flush()
                sink.onKey(vk, meta)
            } else {
                val cp = event.getUnicodeChar(0)
                if (cp == 0) return false
                flush()
                sink.onChar(cp, meta)
            }
            resetBuffer()
            return true
        }
        return false
    }

    private companion object {
        /** Acima disso o campo recomeça (o diff fica sempre pequeno e rápido). */
        const val MAX_BUFFER = 600
    }
}
