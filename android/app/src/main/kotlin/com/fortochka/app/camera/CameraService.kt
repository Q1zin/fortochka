package com.fortochka.app.camera

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.PowerManager
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
 * Foreground-сервис роли «камера»: держит CameraX привязанным к своему
 * жизненному циклу и раз в интервал снимает кадр → отдаёт Rust-ядру
 * на загрузку. Партишн-wakelock позволяет работать при погашенном экране.
 */
class CameraService : LifecycleService() {

    private var wakeLock: PowerManager.WakeLock? = null
    private var imageCapture: ImageCapture? = null

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

        running = true
        lifecycleScope.launch { captureLoop() }
    }

    override fun onDestroy() {
        running = false
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

        try {
            bindCamera()
        } catch (e: Exception) {
            CameraStatus.text.value = "не удалось открыть камеру: ${e.message}"
            stopSelf()
            return
        }

        while (true) {
            try {
                val jpeg = takeJpeg()
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

    private suspend fun bindCamera() {
        val provider = suspendCancellableCoroutine<ProcessCameraProvider> { cont ->
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
        val capture = ImageCapture.Builder()
            .setCaptureMode(ImageCapture.CAPTURE_MODE_MINIMIZE_LATENCY)
            .build()
        provider.unbindAll()
        provider.bindToLifecycle(this, CameraSelector.DEFAULT_BACK_CAMERA, capture)
        imageCapture = capture
    }

    /** Снимок в JPEG-байты: ImageCapture без выходного файла отдаёт готовый JPEG в plane 0. */
    private suspend fun takeJpeg(): ByteArray = suspendCancellableCoroutine { cont ->
        val capture = imageCapture
        if (capture == null) {
            cont.resumeWithException(IllegalStateException("камера не привязана"))
            return@suspendCancellableCoroutine
        }
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
