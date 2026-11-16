package com.comptus.fueros

import android.util.Base64
import android.webkit.JavascriptInterface
import org.json.JSONObject

/**
 * **The one object a resource's page that runs script is given**
 * (`light-client-requirements.md` §8) [author, 2026-11-15]: `rhtn.request`
 * carries one request to the same resource, through the kernel and the
 * node that served the page, and answers with what came back. It reaches
 * nothing else — not this app, not the device, not another resource —
 * which is the whole of why a page may run at all.
 *
 * **Why an object and not the engine's own fetch**: the engine hands a
 * request's method and path to the screen's client and never its body,
 * so a form that posts, or a script that does, has nowhere to go but
 * here. A page's `fetch` of its own origin still works for a `GET`, and
 * is carried the same way.
 *
 * The answer is one JSON object: `status` (the resource's own HTTP
 * status), `media`, and `body` as base64 — or `error` where the kernel
 * carried nothing.
 */
class ResourceBridge(
    private val by: String?,
    private val node: String,
    private val resource: String,
) {
    @JavascriptInterface
    fun request(method: String, path: String, type: String?, body: String?): String {
        val m = method.uppercase()
        if (m !in listOf("GET", "POST", "PUT", "PATCH", "DELETE")) {
            return JSONObject().put("error", "a method this frame does not carry: $method").toString()
        }
        if (!path.startsWith("/") || path.startsWith("/.rhtn/")) {
            return JSONObject().put("error", "a path is rooted on this resource and not under /.rhtn/").toString()
        }
        val bytes = body?.toByteArray(Charsets.UTF_8)
        val who = by ?: Identities.activeId
            ?: return JSONObject().put("error", "nobody is acting").toString()
        val a = Gateway.requestAs(who, node, resource, m, path, type, bytes)
            ?: return JSONObject().put("error", "the kernel did not carry the request").toString()
        if (a.status.toInt() != 0) {
            return JSONObject().put("error", "not delivered: ${ResourcePage.delivery(a.status)}").toString()
        }
        a.unreadable?.let { return JSONObject().put("error", "the answer could not be read: $it").toString() }
        val p = a.page ?: return JSONObject().put("error", "the resource answered with nothing").toString()
        return JSONObject()
            .put("status", p.httpStatus)
            .put("media", p.media ?: JSONObject.NULL)
            .put("body", Base64.encodeToString(p.body, Base64.NO_WRAP))
            .toString()
    }
}
