package com.fortochka.app.viewer

import android.app.WallpaperColors
import android.app.WallpaperManager
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.RectF
import android.os.Build
import android.os.SystemClock
import android.service.wallpaper.WallpaperService
import android.view.SurfaceHolder
import com.fortochka.app.ConfigStore
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.fortochka_mobile.DeviceRole
import uniffi.fortochka_mobile.fetchWallpaper
import uniffi.fortochka_mobile.viewStatus

/**
 * Живые обои — окно в камеру прямо на рабочем столе. Пока обои видны,
 * раз в минуту спрашиваем сервер, нет ли нового кадра, и плавно сменяем
 * картинку; спрятались — ни одного запроса, батарея не тратится.
 */
class LiveWallpaperService : WallpaperService() {

    override fun onCreateEngine(): Engine = WindowEngine()

    private inner class WindowEngine : Engine() {
        private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
        private val paint = Paint(Paint.FILTER_BITMAP_FLAG)
        private var refreshing: Job? = null
        private var lastRefreshAt = 0L

        private var frame: Bitmap? = null
        private var frameAt: Long? = null
        /** Уходящий кадр: виден под новым, пока тот проявляется. */
        private var previous: Bitmap? = null
        private var fade = 1f
        private var colorsReported = false

        override fun onCreate(surfaceHolder: SurfaceHolder) {
            super.onCreate(surfaceHolder)
            // последний кадр с диска — после перезапуска процесса нет пустого экрана
            frame = BitmapFactory.decodeFile(frameFile().path)
            frameAt = LiveFrame.savedAt(this@LiveWallpaperService)
        }

        override fun onSurfaceRedrawNeeded(holder: SurfaceHolder) {
            super.onSurfaceRedrawNeeded(holder)
            draw()
        }

        override fun onVisibilityChanged(visible: Boolean) {
            refreshing?.cancel()
            if (!visible) return
            draw()
            refreshing = scope.launch {
                while (isActive) {
                    // экран включают десятки раз в час — чаще раза в минуту не спрашиваем
                    val wait = lastRefreshAt + REFRESH_MS - SystemClock.elapsedRealtime()
                    if (wait > 0) delay(wait)
                    refresh()
                    lastRefreshAt = SystemClock.elapsedRealtime()
                }
            }
        }

        override fun onDestroy() {
            scope.cancel()
            super.onDestroy()
        }

        // Цвета системной темы (Material You) считаем по первому кадру и больше
        // не трогаем: иначе вся тема телефона перекрашивалась бы с каждым кадром.
        override fun onComputeColors(): WallpaperColors? =
            if (Build.VERSION.SDK_INT >= 27) frame?.let { WallpaperColors.fromBitmap(it) } else null

        private suspend fun refresh() {
            val holderFrame = surfaceHolder.surfaceFrame
            if (holderFrame.isEmpty) return
            val known = frameAt
            val fresh = withContext(Dispatchers.IO) {
                loadIfNew(known, holderFrame.width(), holderFrame.height())
            } ?: return

            previous = frame
            frame = fresh.bitmap
            frameAt = fresh.takenAt
            fadeIn()
            if (!colorsReported && Build.VERSION.SDK_INT >= 27) {
                colorsReported = true
                notifyColorsChanged()
            }
        }

        /** Плавная смена кадра: новый проявляется поверх старого. */
        private suspend fun fadeIn() {
            val start = SystemClock.uptimeMillis()
            try {
                do {
                    fade = ((SystemClock.uptimeMillis() - start).toFloat() / FADE_MS)
                        .coerceAtMost(1f)
                    draw()
                    delay(FRAME_MS)
                } while (fade < 1f)
            } finally {
                // спрятали посреди перехода — старый кадр не должен залипнуть
                previous = null
                fade = 1f
            }
        }

        private fun draw() {
            val holder = surfaceHolder
            val canvas = runCatching { holder.lockCanvas() }.getOrNull() ?: return
            try {
                canvas.drawColor(Color.BLACK)
                val current = frame
                if (current == null) {
                    drawWaiting(canvas)
                } else {
                    previous?.let { drawCropped(canvas, it, 255) }
                    drawCropped(canvas, current, if (previous == null) 255 else (fade * 255).toInt())
                }
            } finally {
                holder.unlockCanvasAndPost(canvas)
            }
        }

        /** Кадр заполняет экран целиком, лишнее поровну срезается по краям. */
        private fun drawCropped(canvas: Canvas, bitmap: Bitmap, alpha: Int) {
            val scale = maxOf(
                canvas.width.toFloat() / bitmap.width,
                canvas.height.toFloat() / bitmap.height,
            )
            val w = bitmap.width * scale
            val h = bitmap.height * scale
            val left = (canvas.width - w) / 2
            val top = (canvas.height - h) / 2
            paint.alpha = alpha
            canvas.drawBitmap(bitmap, null, RectF(left, top, left + w, top + h), paint)
        }

        private fun drawWaiting(canvas: Canvas) {
            val text = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                color = Color.GRAY
                textAlign = Paint.Align.CENTER
                textSize = canvas.width / 22f
            }
            canvas.drawText("Форточка ждёт кадр с камеры", canvas.width / 2f, canvas.height / 2f, text)
        }
    }

    private class Fresh(val takenAt: Long, val bitmap: Bitmap)

    /** Новый кадр — или null: тот же самый, роль не зритель, сеть моргнула. */
    private fun loadIfNew(known: Long?, width: Int, height: Int): Fresh? = runCatching {
        val config = ConfigStore.load(this)
        val baseUrl = config.serverUrl
        val token = config.viewToken
        if (config.role != DeviceRole.VIEWER || baseUrl == null || token == null) return null
        val takenAt = viewStatus(baseUrl, token).lastFrameAt ?: return null
        if (takenAt == known) return null

        val jpeg = fetchWallpaper(baseUrl, token, width.toUInt(), height.toUInt())
        val bitmap = BitmapFactory.decodeByteArray(jpeg, 0, jpeg.size) ?: return null
        LiveFrame.save(this, jpeg, takenAt)
        Fresh(takenAt, bitmap)
    }.getOrNull()

    private fun frameFile(): File = LiveFrame.file(this)

    companion object {
        private const val REFRESH_MS = 60_000L
        private const val FADE_MS = 800f
        private const val FRAME_MS = 16L

        /** На рабочем столе наши живые обои — кадры или видео. */
        fun isActive(context: Context): Boolean =
            WallpaperManager.getInstance(context).wallpaperInfo?.packageName == context.packageName

        /** Системный экран предпросмотра — поставить обои может только человек. */
        fun pickerIntent(context: Context): Intent =
            Intent(WallpaperManager.ACTION_CHANGE_LIVE_WALLPAPER).putExtra(
                WallpaperManager.EXTRA_LIVE_WALLPAPER_COMPONENT,
                ComponentName(context, LiveWallpaperService::class.java),
            )
    }
}

/** Последний кадр живых обоев на диске: показать сразу, а не после сети. */
private object LiveFrame {
    private const val PREFS = "fortochka"
    private const val KEY_AT = "live_frame_at"

    fun file(context: Context) = File(context.filesDir, "live_frame.jpg")

    fun savedAt(context: Context): Long? =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getLong(KEY_AT, -1)
            .takeIf { it >= 0 }

    /** tmp + rename — обои никогда не прочитают половину файла. */
    fun save(context: Context, jpeg: ByteArray, takenAt: Long) {
        val tmp = File(context.filesDir, "live_frame.jpg.tmp")
        tmp.writeBytes(jpeg)
        tmp.renameTo(file(context))
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit()
            .putLong(KEY_AT, takenAt).apply()
    }
}
