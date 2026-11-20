package com.comptus.fueros

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder

/**
 * **The foreground service that is the client's standing on Android**
 * ([Standing]): it holds nothing itself — the kernel's loops run where
 * they always ran — and exists so the platform counts this process as
 * one the person wants kept, with the one notification that costs. Its
 * type is messaging, which Android does not time-limit as it does data
 * sync. A refusal at any step is an outcome [Standing] records, never a
 * crash.
 */
class KernelService : Service() {
    override fun onCreate() {
        super.onCreate()
        Standing.take {
            val manager = getSystemService(NotificationManager::class.java)
                ?: throw IllegalStateException("no notification manager")
            manager.createNotificationChannel(
                NotificationChannel(CHANNEL, "Sessions", NotificationManager.IMPORTANCE_MIN).apply {
                    description = "Fueros keeps your sessions open while you are away."
                    setShowBadge(false)
                },
            )
            val open = Intent(this, ShellActivity::class.java)
                .setFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
            val tap = android.app.PendingIntent.getActivity(
                this,
                0,
                open,
                android.app.PendingIntent.FLAG_UPDATE_CURRENT or android.app.PendingIntent.FLAG_IMMUTABLE,
            )
            val built = Notification.Builder(this, CHANNEL)
                .setSmallIcon(android.R.drawable.ic_menu_rotate)
                .setContentTitle("Fueros")
                .setContentText("Your sessions stay open.")
                .setContentIntent(tap)
                .setOngoing(true)
                .build()
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                startForeground(ID, built, ServiceInfo.FOREGROUND_SERVICE_TYPE_REMOTE_MESSAGING)
            } else {
                // the type has no constant below API 34; the two-argument
                // start takes the manifest's, which is the same type
                startForeground(ID, built)
            }
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int = START_STICKY

    override fun onBind(intent: Intent?): IBinder? = null

    companion object {
        const val CHANNEL: String = "sessions"
        const val ID: Int = 7447

        /**
         * Ask the platform to start the service: the first act that can
         * be refused (a start from the background is, on this platform),
         * taken through [Standing] like the rest.
         */
        fun request(context: Context) {
            Standing.take {
                context.startForegroundService(Intent(context, KernelService::class.java))
            }
        }
    }
}
