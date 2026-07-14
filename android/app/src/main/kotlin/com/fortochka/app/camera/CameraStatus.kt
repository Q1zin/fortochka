package com.fortochka.app.camera

import kotlinx.coroutines.flow.MutableStateFlow

/** Статус сервиса камеры для UI: сервис пишет, экран подписывается. */
object CameraStatus {
    val text = MutableStateFlow("камера не запущена")
}
