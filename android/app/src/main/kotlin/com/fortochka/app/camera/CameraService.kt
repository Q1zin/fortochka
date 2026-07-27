package com.fortochka.app.camera

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.PowerManager
import android.view.OrientationEventListener
import android.view.Surface
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageCapture
import androidx.camera.core.ImageCaptureException
import androidx.camera.core.ImageProxy
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import androidx.lifecycle.LifecycleService
import androidx.lifecycle.lifecycleScope
import com.fortochka.app.ConfigStore
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import uniffi.fortochka_mobile.uploadFrame

/**
 * Foreground-сервис роли «камера»: раз в интервал открывает камеру,
 * снимает кадр и отдаёт его Rust-ядру на загрузку. Партишн-wakelock
 * позволяет работать при погашенном экране.
 */
class CameraService : LifecycleService() {

    private var wakeLock: PowerManager.WakeLock? = null

    /** Поворот «как у экрана» для текущего положения телефона. */
    private var rotation = Surface.ROTATION_0

    /**
     * Экран у камеры не крутится (UI нет, автоповорот обычно выключен),
     * а без подсказки CameraX пишет в EXIF поворот дисплея — и кадр
     * с телефона, стоящего боком, ляжет набок. Положение берём
     * с акселерометра; маппинг — как в документации CameraX.
     */
    private val orientationListener by lazy {
        object : OrientationEventListener(this@CameraService) {
            override fun onOrientationChanged(orientation: Int) {
                if (orientation == ORIENTATION_UNKNOWN) return // лежит плашмя
                rotation = when (orientation) {
                    in 45..134 -> Surface.ROTATION_270
                    in 135..224 -> Surface.ROTATION_180
                    in 225..314 -> Surface.ROTATION_90
                    else -> Surface.ROTATION_0
                }
            }
        }
    }

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        val notification = NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("Форточка — камера")
            .setContentText("Снимаю кадр каждые несколько минут")
            .setSmallIcon(android.R.drawable.ic_menu_camera)
            .setOngoing(true)
            .build()
        ServiceCompat.startForeground(
            this,
            NOTIFICATION_ID,
            notification,
            if (Build.VERSION.SDK_INT >= 29) ServiceInfo.FOREGROUND_SERVICE_TYPE_CAMERA else 0,
        )

        wakeLock = (getSystemService(POWER_SERVICE) as PowerManager)
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "fortochka:camera")
            .apply { acquire() }

        orientationListener.enable()
        running = true
        lifecycleScope.launch { captureLoop() }
    }

    override fun onDestroy() {
        running = false
        orientationListener.disable()
        wakeLock?.release()
        CameraStatus.text.value = "камера остановлена"
        super.onDestroy()
    }

    private suspend fun captureLoop() {
        val config = withContext(Dispatchers.IO) { ConfigStore.load(this@CameraService) }
        val baseUrl = config.serverUrl
        val cameraId = config.cameraId
        val token = config.uploadToken
        if (baseUrl == null || cameraId == null || token == null) {
            CameraStatus.text.value = "камера не настроена — сначала регистрация"
            stopSelf()
            return
        }
        val intervalMs = (config.captureIntervalSecs ?: DEFAULT_INTERVAL_SECS).toLong() * 1000L

        val provider = try {
            cameraProvider()
        } catch (e: Exception) {
            CameraStatus.text.value = "не удалось открыть камеру: ${e.message}"
            stopSelf()
            return
        }

        while (true) {
            try {
                val jpeg = shoot(provider)
                withContext(Dispatchers.IO) { uploadFrame(baseUrl, cameraId, token, jpeg) }
                CameraStatus.text.value = "кадр отправлен в ${timeNow()} (${jpeg.size / 1024} КБ)"
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                CameraStatus.text.value = "ошибка в ${timeNow()}: ${e.message}"
            }
            // delay вне try: отмена корутины (стоп сервиса) выходит из цикла
            delay(intervalMs)
        }
    }

    private suspend fun cameraProvider(): ProcessCameraProvider =
        suspendCancellableCoroutine { cont ->
            val future = ProcessCameraProvider.getInstance(this)
            future.addListener(
                {
                    try {
                        cont.resume(future.get())
                    } catch (e: Exception) {
                        cont.resumeWithException(e)
                    }
                },
                ContextCompat.getMainExecutor(this),
            )
        }

    /**
     * Камера открыта только на время снимка: держать сенсор включённым
     * ради кадра раз в несколько минут — нагрев, батарея и занятая камера
     * для остальных (на Samsung — сенсор автояркости).
     */
    private suspend fun shoot(provider: ProcessCameraProvider): ByteArray {
        val capture = ImageCapture.Builder()
            .setCaptureMode(ImageCapture.CAPTURE_MODE_MINIMIZE_LATENCY)
            .setTargetRotation(rotation)
            // сервер всё равно пережимает кадр в q85 — больше слать незачем
            .setJpegQuality(JPEG_QUALITY)
            .build()
        provider.unbindAll()
        provider.bindToLifecycle(this, CameraSelector.DEFAULT_BACK_CAMERA, capture)
        try {
            // только что открытой камере нужно время на экспозицию и баланс белого
            delay(WARMUP_MS)
            return takeJpeg(capture)
        } finally {
            provider.unbindAll()
        }
    }

    /** Снимок в JPEG-байты: ImageCapture без выходного файла отдаёт готовый JPEG в plane 0. */
    private suspend fun takeJpeg(capture: ImageCapture): ByteArray =
        suspendCancellableCoroutine { cont ->
            capture.takePicture(
                ContextCompat.getMainExecutor(this),
                object : ImageCapture.OnImageCapturedCallback() {
                    override fun onCaptureSuccess(image: ImageProxy) {
                        try {
                            val buffer = image.planes[0].buffer
                            val bytes = ByteArray(buffer.remaining())
                            buffer.get(bytes)
                            cont.resume(bytes)
                        } finally {
                            image.close()
                        }
                    }

                    override fun onError(exception: ImageCaptureException) {
                        cont.resumeWithException(exception)
                    }
                },
            )
        }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT < 26) return
        val channel = NotificationChannel(
            CHANNEL_ID,
            "Камера Форточки",
            NotificationManager.IMPORTANCE_LOW,
        )
        (getSystemService(NOTIFICATION_SERVICE) as NotificationManager)
            .createNotificationChannel(channel)
    }

    private fun timeNow(): String =
        SimpleDateFormat("HH:mm:ss", Locale.getDefault()).format(Date())

    companion object {
        private const val CHANNEL_ID = "fortochka_camera"
        private const val NOTIFICATION_ID = 1
        private const val WARMUP_MS = 2_000L
        private const val JPEG_QUALITY = 85
        const val DEFAULT_INTERVAL_SECS = 180u

        @Volatile
        var running = false
            private set

        fun start(context: Context) = ContextCompat.startForegroundService(
            context,
            Intent(context, CameraService::class.java),
        )

        fun stop(context: Context) {
            context.stopService(Intent(context, CameraService::class.java))
        }
    }
}
