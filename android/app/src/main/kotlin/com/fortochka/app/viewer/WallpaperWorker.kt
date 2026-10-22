package com.fortochka.app.viewer

import android.app.WallpaperManager
import android.content.Context
import android.graphics.BitmapFactory
import android.hardware.display.DisplayManager
import android.os.Build
import android.view.Display
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import androidx.work.workDataOf
import com.fortochka.app.ConfigStore
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.fortochka_mobile.DeviceRole
import uniffi.fortochka_mobile.fetchWallpaper
import uniffi.fortochka_mobile.viewStatus

/**
 * Тянет свежий кадр (сервер сам кропит под разрешение экрана)
 * и ставит его как обои на домашний экран и локскрин.
 * 15 минут — минимальный интервал периодических задач WorkManager.
 */
class WallpaperWorker(context: Context, params: WorkerParameters) :
    CoroutineWorker(context, params) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        val config = ConfigStore.load(applicationContext)
        val baseUrl = config.serverUrl
        val viewToken = config.viewToken
        if (config.role != DeviceRole.VIEWER || baseUrl == null || viewToken == null) {
            return@withContext Result.failure()
        }

        try {
            // Сначала дешёвый статус: если на обоях уже этот кадр — не качаем
            // его и не трогаем обои. «Обновить сейчас» ставит force.
            val frameAt = viewStatus(baseUrl, viewToken).lastFrameAt
            if (frameAt == null) {
                ViewerStatus.update(applicationContext, "ждём первый кадр от камеры")
                return@withContext Result.retry()
            }
            val force = inputData.getBoolean(KEY_FORCE, false)
            if (!force && frameAt == ViewerStatus.appliedFrameAt(applicationContext)) {
                ViewerStatus.update(
                    applicationContext,
                    "новых кадров нет, на обоях кадр ${clock(frameAt, nowSecs())}",
                )
                return@withContext Result.success()
            }

            // Весь экран в естественной ориентации: displayMetrics не включают
            // системные панели (на Galaxy M21 — 1080×2131 вместо 1080×2340)
            // и зависят от того, как повёрнут экран в момент запуска.
            val mode = applicationContext.getSystemService(DisplayManager::class.java)
                .getDisplay(Display.DEFAULT_DISPLAY).mode
            val jpeg = fetchWallpaper(
                baseUrl,
                viewToken,
                mode.physicalWidth.toUInt(),
                mode.physicalHeight.toUInt(),
            )
            val bitmap = BitmapFactory.decodeByteArray(jpeg, 0, jpeg.size)
                ?: error("сервер вернул не картинку")

            val wm = WallpaperManager.getInstance(applicationContext)
            if (Build.VERSION.SDK_INT >= 24) {
                wm.setBitmap(
                    bitmap,
                    null,
                    true,
                    WallpaperManager.FLAG_SYSTEM or WallpaperManager.FLAG_LOCK,
                )
            } else {
                @Suppress("DEPRECATION")
                wm.setBitmap(bitmap)
            }
            ViewerStatus.setApplied(applicationContext, frameAt)
            ViewerStatus.update(
                applicationContext,
                "обои обновлены в ${timeNow()}, кадр ${clock(frameAt, nowSecs())}",
            )
            Result.success()
        } catch (e: Exception) {
            ViewerStatus.update(applicationContext, "ошибка в ${timeNow()}: ${e.message}")
            // сеть могла моргнуть — WorkManager повторит с backoff
            Result.retry()
        }
    }

    private fun timeNow(): String =
        SimpleDateFormat("HH:mm", Locale.getDefault()).format(Date())

    private fun nowSecs(): Long = System.currentTimeMillis() / 1000

    companion object {
        private const val PERIODIC_NAME = "fortochka-wallpaper"
        private const val KEY_FORCE = "force"

        fun schedule(context: Context) {
            val request = PeriodicWorkRequestBuilder<WallpaperWorker>(15, TimeUnit.MINUTES)
                .setConstraints(
                    Constraints.Builder()
                        .setRequiredNetworkType(NetworkType.CONNECTED)
                        .build(),
                )
                .build()
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(
                PERIODIC_NAME,
                ExistingPeriodicWorkPolicy.UPDATE,
                request,
            )
        }

        /** Ручное обновление: ставим кадр, даже если он уже на обоях. */
        fun refreshNow(context: Context) {
            WorkManager.getInstance(context).enqueue(
                OneTimeWorkRequestBuilder<WallpaperWorker>()
                    .setInputData(workDataOf(KEY_FORCE to true))
                    .build(),
            )
        }

        fun cancel(context: Context) {
            WorkManager.getInstance(context).cancelUniqueWork(PERIODIC_NAME)
        }
    }
}
