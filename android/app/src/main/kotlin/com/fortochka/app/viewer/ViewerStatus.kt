package com.fortochka.app.viewer

import android.content.Context
import kotlinx.coroutines.flow.MutableStateFlow

/**
 * Статус зрителя. В отличие от камеры, воркер живёт вне UI-процесса
 * по времени (запускается системой), поэтому последний результат
 * дублируется в SharedPreferences и восстанавливается при старте UI.
 */
object ViewerStatus {
    private const val PREFS = "fortochka"
    private const val KEY = "viewer_last_status"

    val text = MutableStateFlow("ждём первого обновления обоев")

    fun restore(context: Context) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .getString(KEY, null)
            ?.let { text.value = it }
    }

    fun update(context: Context, message: String) {
        text.value = message
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putString(KEY, message)
            .apply()
    }
}
