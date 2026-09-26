package com.marcelohenrique.tecladoremoto

import android.annotation.TargetApi
import android.app.Activity
import android.window.OnBackInvokedCallback
import android.window.OnBackInvokedDispatcher

/** "Voltar" preditivo do Android 13+ (isolado para não carregar a classe em versões antigas). */
@TargetApi(33)
object BackCompat {
    fun register(activity: Activity, onBack: () -> Unit): Any {
        val callback = OnBackInvokedCallback { onBack() }
        activity.onBackInvokedDispatcher.registerOnBackInvokedCallback(OnBackInvokedDispatcher.PRIORITY_DEFAULT, callback)
        return callback
    }

    fun unregister(activity: Activity, token: Any) {
        activity.onBackInvokedDispatcher.unregisterOnBackInvokedCallback(token as OnBackInvokedCallback)
    }
}
