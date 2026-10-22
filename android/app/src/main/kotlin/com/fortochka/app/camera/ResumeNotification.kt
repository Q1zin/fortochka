package com.fortochka.app.camera

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.core.app.NotificationCompat

/**
 * Камера встала не по своей воле: Android 15+ не даёт запускать camera-сервис
 * из BOOT_COMPLETED, после обновления приложения сервис сам не поднимается.
 * Нужен один тап человека — показываем уведомление, тап запускает съёмку.
 */
object ResumeNotification {
    private const val CHANNEL_ID = "fortochka_resume"
    private const val NOTIFICATION_ID = 2

    fun show(context: Context) {
        val manager = context.getSystemService(NotificationManager::class.java)
        if (Build.VERSION.SDK_INT >= 26) {
            manager.createNotificationChannel(
                NotificationChannel(
                    CHANNEL_ID,
                    "Перезапуск камеры",
                    NotificationManager.IMPORTANCE_DEFAULT,
                ),
            )
        }
        // тап по уведомлению — законный способ поднять camera-сервис из фона
        val intent = Intent(context, CameraService::class.java)
        val tap = if (Build.VERSION.SDK_INT >= 26) {
            PendingIntent.getForegroundService(context, 0, intent, PendingIntent.FLAG_IMMUTABLE)
        } else {
            PendingIntent.getService(context, 0, intent, PendingIntent.FLAG_IMMUTABLE)
        }
        val notification = NotificationCompat.Builder(context, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.ic_menu_camera)
            .setContentTitle("Форточка: камера на паузе")
            .setContentText("Нажмите, чтобы продолжить съёмку")
            .setContentIntent(tap)
            .setAutoCancel(true)
            .build()
        manager.notify(NOTIFICATION_ID, notification)
    }

    fun hide(context: Context) {
        context.getSystemService(NotificationManager::class.java).cancel(NOTIFICATION_ID)
    }
}
