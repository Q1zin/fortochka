package com.fortochka.app.camera

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.Build
import com.fortochka.app.ConfigStore
import uniffi.fortochka_mobile.DeviceRole

/**
 * Телефон-камера стоит стационарно — после ребута и обновления приложения
 * съёмка должна продолжиться. После ребута до Android 15 сервис поднимается
 * сам (свет отключили на даче — камера вернётся без людей); на 15+ camera-сервис
 * из BOOT_COMPLETED запрещён, а после обновления человек и так рядом — просим тап.
 */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val action = intent.action
        if (action != Intent.ACTION_BOOT_COMPLETED && action != Intent.ACTION_MY_PACKAGE_REPLACED) {
            return
        }
        if (ConfigStore.load(context).role != DeviceRole.CAMERA) return

        if (action == Intent.ACTION_BOOT_COMPLETED && Build.VERSION.SDK_INT < 35) {
            CameraService.start(context)
        } else {
            ResumeNotification.show(context)
        }
    }
}
