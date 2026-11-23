package com.comptus.fueros

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent

/**
 * **A resource's notification, shown** (`light-client-requirements.md`
 * §8.1's `notify`).
 *
 * **What this is and what it is not.** The standing permission is the
 * person's, given through the ask and kept by this shell per resource
 * ([Giving.NOTIFY_STORE]); what this file does is *present* a resource's
 * ask in the platform's own shade, so a person who has left the screen
 * still learns that something waits. An ask is learned by asking; what a
 * resource counts as waiting arrives on the page its node carries, which
 * the node says has changed over the session this client keeps [author,
 * 2026-11-23] ([notices]). Nothing here claims to have been pushed: what
 * is posted is what has already arrived.
 *
 * **It carries no content of the resource's beyond what the person is
 * about to be shown anyway**: the utility and the purpose in the
 * resource's own words, which is what the consent screen shows. Tapping
 * it opens the resource's own screen, where the ask is answered.
 */
object Notify {

    /** The one channel a resource's notifications go to, so the person can
     *  silence every resource at once in the platform's own settings. */
    const val CHANNEL: String = "resources"

    /** Whether this resource holds the person's standing permission. */
    fun allowed(context: Context, resource: String): Boolean =
        runCatching {
            context.getSharedPreferences(Giving.NOTIFY_STORE, Context.MODE_PRIVATE)
                .getBoolean(resource, false)
        }.getOrDefault(false)

    /** Take the permission back, which the person may do at any time. */
    fun forget(context: Context, resource: String) {
        runCatching {
            context.getSharedPreferences(Giving.NOTIFY_STORE, Context.MODE_PRIVATE)
                .edit()
                .remove(resource)
                .apply()
        }
    }

    /**
     * **The title and the line**, as a person reads them: who it is from
     * is a resource's keyhash and means nothing on a notification, so the
     * utility is the title and the resource's own purpose is the line.
     */
    fun words(ask: Gateway.Ask): Pair<String, String> {
        val title = when (ask.utility) {
            "notify" -> "A resource asks to notify you"
            "export" -> "A resource has a file for you"
            "import" -> "A resource asks for a file"
            "sign" -> "A resource asks you to sign something"
            "location" -> "A resource asks where you are"
            "capture", "scan" -> "A resource asks for a photograph"
            "tag", "check-in" -> "A resource asks you to read a tag"
            "live" -> "A resource asks for a live session"
            else -> "A resource asks something of you"
        }
        return title to ask.purpose
    }

    /** Where the count each resource was last seen at is kept, per identity. */
    const val SEEN_STORE: String = "notices-seen"

    /**
     * **Which resources have more waiting than when last seen, with a line
     * to say so** [author, 2026-11-23]: the count is the resource's own, a
     * rise is something new, and a resource that gave no line has nothing
     * to put in a notification.
     */
    fun rising(seen: Map<String, Int>, held: List<uniffi.rhtn_model.Resource>): List<uniffi.rhtn_model.Resource> =
        held.filter { it.notice != null && it.waiting.toInt() > (seen[it.resource.hex()] ?: 0) }

    /**
     * **Tell the shade what a resource counts as newly waiting**, for each
     * resource holding the person's permission, while no screen of this
     * app shows it; and keep every count as seen, so the next rise is
     * measured from it and a fall is not news.
     */
    fun notices(context: Context, by: String, held: List<uniffi.rhtn_model.Resource>) {
        val store = runCatching { context.getSharedPreferences("$SEEN_STORE-$by", Context.MODE_PRIVATE) }
            .getOrNull() ?: return
        val seen = held.associate { it.resource.hex() to store.getInt(it.resource.hex(), 0) }
        if (!Arrived.onScreen()) {
            for (r in rising(seen, held)) post(context, by, r)
        }
        store.edit().apply { held.forEach { putInt(it.resource.hex(), it.waiting.toInt()) } }.apply()
    }

    /** Post one resource's notice: its name, its line, and opening it on a tap. */
    private fun post(context: Context, by: String, r: uniffi.rhtn_model.Resource): Boolean {
        val resource = r.resource.hex()
        if (!allowed(context, resource)) return false
        if (android.os.Build.VERSION.SDK_INT >= 33 &&
            context.checkSelfPermission("android.permission.POST_NOTIFICATIONS") !=
            android.content.pm.PackageManager.PERMISSION_GRANTED
        ) {
            return false
        }
        val manager = context.getSystemService(NotificationManager::class.java) ?: return false
        return runCatching {
            manager.createNotificationChannel(
                NotificationChannel(CHANNEL, "Resources", NotificationManager.IMPORTANCE_DEFAULT)
                    .apply { description = "What resources you use ask of this device." },
            )
            val open = Intent(context, ResourceActivity::class.java)
                .putExtra("by", by)
                .putExtra("node", r.node.hex())
                .putExtra("resource", resource)
                .putExtra("name", r.instance)
                .putExtra("path", "/")
                .setFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
            val tap = PendingIntent.getActivity(
                context,
                resource.hashCode(),
                open,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
            val line = r.notice ?: ""
            val built = Notification.Builder(context, CHANNEL)
                .setSmallIcon(android.R.drawable.ic_dialog_email)
                .setContentTitle(r.instance)
                .setContentText(line)
                .setStyle(Notification.BigTextStyle().bigText(line))
                .setContentIntent(tap)
                .setAutoCancel(true)
                .build()
            manager.notify(resource.hashCode(), built)
            Diag.event("notice", "resource" to resource.take(8), "count" to r.waiting.toInt())
            true
        }.getOrDefault(false)
    }

    /**
     * Post the ask, where the resource holds the permission and the
     * platform allows it.  Whether anything was posted, so a screen can
     * say so in a diagnostic line and never pretend.
     */
    fun post(context: Context, ask: Gateway.Ask): Boolean {
        if (!allowed(context, ask.resource)) return false
        if (android.os.Build.VERSION.SDK_INT >= 33 &&
            context.checkSelfPermission("android.permission.POST_NOTIFICATIONS") !=
            android.content.pm.PackageManager.PERMISSION_GRANTED
        ) {
            return false
        }
        val manager = context.getSystemService(NotificationManager::class.java) ?: return false
        return runCatching {
            manager.createNotificationChannel(
                NotificationChannel(
                    CHANNEL,
                    "Resources",
                    NotificationManager.IMPORTANCE_DEFAULT,
                ).apply {
                    description = "What resources you use ask of this device."
                },
            )
            val (title, line) = words(ask)
            val open = Intent(context, ResourceActivity::class.java)
                .putExtra("node", ask.node)
                .putExtra("resource", ask.resource)
                .putExtra("name", "A resource")
                .putExtra("path", "/")
                .setFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
            val tap = PendingIntent.getActivity(
                context,
                ask.resource.hashCode(),
                open,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
            val built = Notification.Builder(context, CHANNEL)
                .setSmallIcon(android.R.drawable.ic_dialog_email)
                .setContentTitle(title)
                .setContentText(line)
                .setStyle(Notification.BigTextStyle().bigText(line))
                .setContentIntent(tap)
                .setAutoCancel(true)
                .build()
            // one per resource, replaced rather than stacked: a second ask
            // from the same resource is the same waiting thing
            manager.notify(ask.resource.hashCode(), built)
            Diag.event("utility", "which" to ask.utility, "state" to "notified")
            true
        }.getOrDefault(false)
    }
}
