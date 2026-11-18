package com.comptus.fueros

import android.app.ActivityManager
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent

/**
 * **A message that arrived while no screen of this app is showing, told in
 * the platform's shade** (design §14.1.4, MAIL-029) [author, 2026-11-17]:
 * *"Use the on-node notification service for routine notifications."* The
 * message came over the session the client keeps ([KernelService]), so no
 * push service is involved; this is only the telling.
 *
 * **Who it is from, never what it says**: the shade is read by whoever
 * holds the phone, and the thread is one tap away.
 */
object Arrived {

    /** The one channel messages go to, so the person can silence them in
     *  the platform's own settings apart from the sessions' standing. */
    const val CHANNEL: String = "messages"

    /** Whether a screen of this app is in front of the person, where the
     *  thread itself shows the message and the shade would repeat it. The
     *  foreground service alone ranks below this. */
    fun onScreen(): Boolean {
        val info = ActivityManager.RunningAppProcessInfo()
        ActivityManager.getMyMemoryState(info)
        return info.importance == ActivityManager.RunningAppProcessInfo.IMPORTANCE_FOREGROUND
    }

    /** Post it, where the platform allows. Whether anything was posted. */
    fun post(context: Context, who: String, peerKey: String): Boolean {
        if (android.os.Build.VERSION.SDK_INT >= 33 &&
            context.checkSelfPermission("android.permission.POST_NOTIFICATIONS") !=
            android.content.pm.PackageManager.PERMISSION_GRANTED
        ) {
            return false
        }
        return runCatching {
            val manager = context.getSystemService(NotificationManager::class.java) ?: return false
            manager.createNotificationChannel(
                NotificationChannel(CHANNEL, "Messages", NotificationManager.IMPORTANCE_DEFAULT),
            )
            val open = Intent(context, ShellActivity::class.java)
                .setFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
            val tap = PendingIntent.getActivity(
                context,
                peerKey.hashCode(),
                open,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
            val built = Notification.Builder(context, CHANNEL)
                .setSmallIcon(android.R.drawable.sym_action_chat)
                .setContentTitle(who)
                .setContentText("A message")
                .setContentIntent(tap)
                .setAutoCancel(true)
                .build()
            manager.notify(peerKey.hashCode(), built)
            true
        }.getOrDefault(false)
    }
}
