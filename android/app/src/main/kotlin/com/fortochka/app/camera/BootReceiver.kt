package com.fortochka.app.camera

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import com.fortochka.app.ConfigStore
import uniffi.fortochka_mobile.DeviceRole

/** Телефон-камера стоит стационарно — после ребута сервис поднимается сам. */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Intent.ACTION_BOOT_COMPLETED) return
        val config = ConfigStore.load(context)
        if (config.role == DeviceRole.CAMERA) {
            CameraService.start(context)
        }
    }
}
