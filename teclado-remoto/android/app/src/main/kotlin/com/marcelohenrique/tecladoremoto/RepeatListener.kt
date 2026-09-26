package com.marcelohenrique.tecladoremoto

import android.annotation.SuppressLint
import android.view.MotionEvent
import android.view.View

/** Segurar o botão repete a tecla, como num teclado físico. */
class RepeatListener(private val onPress: (View) -> Unit) : View.OnTouchListener {
    private var target: View? = null
    private val repeater = object : Runnable {
        override fun run() {
            val view = target ?: return
            onPress(view)
            view.postDelayed(this, REPEAT_MS)
        }
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouch(view: View, event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                target = view
                view.drawableHotspotChanged(event.x, event.y)
                view.isPressed = true
                onPress(view)
                view.postDelayed(repeater, FIRST_DELAY_MS)
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                view.removeCallbacks(repeater)
                view.isPressed = false
                target = null
            }
        }
        return true
    }

    private companion object {
        const val FIRST_DELAY_MS = 400L
        const val REPEAT_MS = 50L
    }
}
