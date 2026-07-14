package com.fortochka.app.viewer

import android.app.WallpaperManager
import android.content.Context
import android.content.res.Resources
import android.graphics.BitmapFactory
import android.os.Build
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import com.fortochka.app.ConfigStore
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.fortochka_mobile.DeviceRole
import uniffi.fortochka_mobile.fetchWallpaper

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
            val dm = Resources.getSystem().displayMetrics
            val jpeg = fetchWallpaper(
                baseUrl,
                viewToken,
                dm.widthPixels.toUInt(),
                dm.heightPixels.toUInt(),
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
            ViewerStatus.update(applicationContext, "обои обновлены в ${timeNow()}")
            Result.success()
        } catch (e: Exception) {
            ViewerStatus.update(applicationContext, "ошибка в ${timeNow()}: ${e.message}")
            // сеть могла моргнуть — WorkManager повторит с backoff
            Result.retry()
        }
    }

    private fun timeNow(): String =
        SimpleDateFormat("HH:mm", Locale.getDefault()).format(Date())

    companion object {
        private const val PERIODIC_NAME = "fortochka-wallpaper"

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

        fun refreshNow(context: Context) {
            WorkManager.getInstance(context)
                .enqueue(OneTimeWorkRequestBuilder<WallpaperWorker>().build())
        }

        fun cancel(context: Context) {
            WorkManager.getInstance(context).cancelUniqueWork(PERIODIC_NAME)
        }
    }
}
