package com.marcelohenrique.tecladoremoto

import android.annotation.SuppressLint
import android.content.Context
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import com.marcelohenrique.tecladoremoto.core.Proto
import kotlin.math.abs
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.min

/**
 * Touchpad: um dedo move o ponteiro (com aceleração), toque = clique esquerdo,
 * dois dedos = rolar, toque com dois dedos = clique direito.
 */
class TouchpadView(context: Context, attrs: AttributeSet?) : View(context, attrs) {
    interface Listener {
        fun onPointerMove(dx: Int, dy: Int)
        fun onScroll(vertical: Int, horizontal: Int)
        fun onClick(button: Int)
    }

    var listener: Listener? = null

    private val density = resources.displayMetrics.density
    private val slop = ViewConfiguration.get(context).scaledTouchSlop.toFloat()
    private var downX = 0f
    private var downY = 0f
    private var downTime = 0L
    private var lastX = 0f
    private var lastY = 0f
    private var lastTime = 0L
    private var maxPointers = 0
    private var moved = false
    private var restX = 0f
    private var restY = 0f
    private var restScrollV = 0f
    private var restScrollH = 0f

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                parent?.requestDisallowInterceptTouchEvent(true)
                downX = event.x
                downY = event.y
                downTime = event.eventTime
                maxPointers = 1
                moved = false
                restX = 0f
                restY = 0f
                restScrollV = 0f
                restScrollH = 0f
                remember(event, -1)
            }
            MotionEvent.ACTION_POINTER_DOWN -> {
                maxPointers = max(maxPointers, event.pointerCount)
                remember(event, -1)
            }
            MotionEvent.ACTION_POINTER_UP -> remember(event, event.actionIndex)
            MotionEvent.ACTION_MOVE -> {
                val (x, y) = centroid(event, -1)
                if (!moved && hypot(x - downX, y - downY) > slop) moved = true
                if (moved) {
                    val dt = max(1L, event.eventTime - lastTime)
                    if (event.pointerCount >= 2) scroll(x - lastX, y - lastY) else move(x - lastX, y - lastY, dt)
                }
                lastX = x
                lastY = y
                lastTime = event.eventTime
            }
            MotionEvent.ACTION_UP -> {
                if (!moved && event.eventTime - downTime < TAP_MS) {
                    listener?.onClick(if (maxPointers >= 2) Proto.BUTTON_RIGHT else Proto.BUTTON_LEFT)
                    performHapticFeedback(android.view.HapticFeedbackConstants.KEYBOARD_TAP)
                }
            }
        }
        return true
    }

    private fun centroid(event: MotionEvent, skip: Int): Pair<Float, Float> {
        var sx = 0f
        var sy = 0f
        var n = 0
        for (i in 0 until event.pointerCount) {
            if (i == skip) continue
            sx += event.getX(i)
            sy += event.getY(i)
            n++
        }
        return if (n == 0) event.x to event.y else sx / n to sy / n
    }

    /** Recalcula a referência quando um dedo entra/sai, para o ponteiro não "pular". */
    private fun remember(event: MotionEvent, skip: Int) {
        val (x, y) = centroid(event, skip)
        lastX = x
        lastY = y
        lastTime = event.eventTime
    }

    private fun move(dx: Float, dy: Float, dt: Long) {
        val x = dx / density
        val y = dy / density
        // Devagar = precisão; rápido = atravessa a tela.
        val speed = hypot(x, y) / dt
        val gain = 1.6f + min(speed, 3f) * 1.8f
        restX += x * gain
        restY += y * gain
        val ix = restX.toInt()
        val iy = restY.toInt()
        if (ix != 0 || iy != 0) {
            restX -= ix
            restY -= iy
            listener?.onPointerMove(ix, iy)
        }
    }

    private fun scroll(dx: Float, dy: Float) {
        // Rolagem "natural" como no celular; ~20 dp de arrasto = um clique da roda (120).
        restScrollV += dy / density * 6f
        restScrollH -= dx / density * 6f
        // Só uma direção por vez: rolagem vertical não "escorrega" para os lados.
        if (abs(restScrollH) > abs(restScrollV) * 2) restScrollV = 0f else restScrollH = 0f
        val v = restScrollV.toInt()
        val h = restScrollH.toInt()
        if (abs(v) >= MIN_WHEEL || abs(h) >= MIN_WHEEL) {
            restScrollV -= v
            restScrollH -= h
            listener?.onScroll(v, h)
        }
    }

    private companion object {
        const val TAP_MS = 250L

        /** Rolagem suave em passos de 1/10 de "clique" da roda. */
        const val MIN_WHEEL = 12
    }
}
