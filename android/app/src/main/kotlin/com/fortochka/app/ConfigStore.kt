package com.fortochka.app

import android.content.Context
import uniffi.fortochka_mobile.DeviceConfig
import uniffi.fortochka_mobile.loadConfig
import uniffi.fortochka_mobile.saveConfig

/**
 * Конфиг устройства живёт в Rust-ядре (files-каталог приложения),
 * здесь только обёртка с контекстом. Все вызовы блокирующие —
 * дергать из Dispatchers.IO.
 */
object ConfigStore {
    fun load(context: Context): DeviceConfig = loadConfig(context.filesDir.absolutePath)

    fun save(context: Context, config: DeviceConfig) =
        saveConfig(context.filesDir.absolutePath, config)

    fun reset(context: Context) =
        save(context, DeviceConfig(null, null, null, null, null, null, null))
}
