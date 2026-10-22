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
    private const val KEY_APPLIED = "viewer_applied_frame_at"
    private const val INITIAL = "ждём первого обновления обоев"

    val text = MutableStateFlow(INITIAL)

    /** Время кадра, который уже стоит на обоях: такой же качать незачем. */
    fun appliedFrameAt(context: Context): Long? =
        prefs(context).getLong(KEY_APPLIED, -1).takeIf { it >= 0 }

    fun setApplied(context: Context, frameAt: Long) {
        prefs(context).edit().putLong(KEY_APPLIED, frameAt).apply()
    }

    /** Сброс роли: следующая камера начинает с чистого листа. */
    fun clear(context: Context) {
        prefs(context).edit().remove(KEY).remove(KEY_APPLIED).apply()
        text.value = INITIAL
    }

    private fun prefs(context: Context) =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

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
