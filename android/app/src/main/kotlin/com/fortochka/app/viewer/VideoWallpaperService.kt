package com.fortochka.app.viewer

import android.app.WallpaperManager
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.media.MediaPlayer
import android.net.ConnectivityManager
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
import uniffi.fortochka_mobile.fetchClip
import uniffi.fortochka_mobile.viewStatus

/**
 * Видео-обои: последний ролик с камеры крутится по кругу, пока обои видны.
 * Отдельный сервис, а не режим [LiveWallpaperService]: одну поверхность нельзя
 * рисовать то Canvas'ом, то плеером. Свежий ролик качаем только по безлимитной
 * сети — на мобильном интернете крутится последний скачанный.
 */
class VideoWallpaperService : WallpaperService() {

    override fun onCreateEngine(): Engine = ClipEngine()

    private inner class ClipEngine : Engine() {
        private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
        private var refreshing: Job? = null
        private var lastRefreshAt = 0L
        private var player: MediaPlayer? = null
        /** start() до onPrepared ломает MediaPlayer — ждём готовности. */
        private var prepared = false
        private var shown = false

        override fun onSurfaceCreated(holder: SurfaceHolder) {
            super.onSurfaceCreated(holder)
            play(holder)
        }

        override fun onSurfaceDestroyed(holder: SurfaceHolder) {
            refreshing?.cancel()
            release()
            super.onSurfaceDestroyed(holder)
        }

        override fun onVisibilityChanged(visible: Boolean) {
            shown = visible
            refreshing?.cancel()
            if (!visible) {
                player?.takeIf { it.isPlaying }?.pause()
                return
            }
            if (prepared) player?.start()
            refreshing = scope.launch {
                while (isActive) {
                    // экран включают десятки раз в час — чаще раза в минуту не спрашиваем
                    val wait = lastRefreshAt + REFRESH_MS - SystemClock.elapsedRealtime()
                    if (wait > 0) delay(wait)
                    val fresh = withContext(Dispatchers.IO) {
                        downloadIfNew(this@VideoWallpaperService, onlyUnmetered = true)
                    }
                    lastRefreshAt = SystemClock.elapsedRealtime()
                    if (fresh) play(surfaceHolder)
                }
            }
        }

        override fun onDestroy() {
            scope.cancel()
            release()
            super.onDestroy()
        }

        /** Ролик с диска — по кругу, без звука, на весь экран с обрезкой краёв. */
        private fun play(holder: SurfaceHolder) {
            release()
            val file = clipFile(this@VideoWallpaperService)
            if (!file.exists()) return
            player = MediaPlayer().apply {
                setSurface(holder.surface)
                setDataSource(file.path)
                isLooping = true
                setVolume(0f, 0f)
                setOnPreparedListener {
                    prepared = true
                    // режим масштаба действует только после того, как плеер готов
                    it.setVideoScalingMode(MediaPlayer.VIDEO_SCALING_MODE_SCALE_TO_FIT_WITH_CROPPING)
                    if (shown) it.start()
                }
                prepareAsync()
            }
        }

        private fun release() {
            prepared = false
            player?.release()
            player = null
        }
    }

    companion object {
        private const val REFRESH_MS = 60_000L
        private const val PREFS = "fortochka"
        private const val KEY_AT = "live_clip_at"

        fun clipFile(context: Context) = File(context.filesDir, "live_clip.mp4")

        /**
         * Скачать ролик, если на сервере есть новее того, что на диске.
         * Блокирующая — звать с IO. Ошибки сети глотаем: обои крутят старый ролик.
         */
        fun downloadIfNew(context: Context, onlyUnmetered: Boolean): Boolean = runCatching {
            val network = context.getSystemService(ConnectivityManager::class.java)
            if (onlyUnmetered && network.isActiveNetworkMetered) return false
            val config = ConfigStore.load(context)
            val baseUrl = config.serverUrl
            val token = config.viewToken
            if (config.role != DeviceRole.VIEWER || baseUrl == null || token == null) return false

            val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            val clipAt = viewStatus(baseUrl, token).lastClipAt ?: return false
            if (clipAt == prefs.getLong(KEY_AT, -1) && clipFile(context).exists()) return false
            fetchClip(baseUrl, token, clipFile(context).path)
            prefs.edit().putLong(KEY_AT, clipAt).apply()
            true
        }.getOrDefault(false)

        /** Системный предпросмотр видео-обоев — поставить их может только человек. */
        fun pickerIntent(context: Context): Intent =
            Intent(WallpaperManager.ACTION_CHANGE_LIVE_WALLPAPER).putExtra(
                WallpaperManager.EXTRA_LIVE_WALLPAPER_COMPONENT,
                ComponentName(context, VideoWallpaperService::class.java),
            )
    }
}
