package com.fortochka.app.viewer

import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/** Камера снимает раз в 3 минуты: 20 минут тишины — уже повод сказать зрителю. */
const val CAMERA_SILENT_AFTER_SECS = 20 * 60L

/** Строка для экрана зрителя: жива ли камера и когда она прислала кадр. */
fun describeCamera(lastFrameAt: Long?, nowSecs: Long): String = when {
    lastFrameAt == null -> "Камера ещё не прислала ни одного кадра"
    nowSecs - lastFrameAt <= CAMERA_SILENT_AFTER_SECS ->
        "Камера на связи, кадр в ${clock(lastFrameAt, nowSecs)}"
    else -> "Камера молчит с ${clock(lastFrameAt, nowSecs)}"
}

/** «14:05» для сегодняшнего кадра, «21.10 14:05» — для более старого. */
fun clock(unixSecs: Long, nowSecs: Long): String {
    val day = SimpleDateFormat("yyyyMMdd", Locale.ROOT)
    val today = day.format(Date(unixSecs * 1000)) == day.format(Date(nowSecs * 1000))
    return SimpleDateFormat(if (today) "HH:mm" else "dd.MM HH:mm", Locale.ROOT)
        .format(Date(unixSecs * 1000))
}
