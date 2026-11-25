// rhtn.request, as the desktop publishes it (rhtn_desktop::frame::bridge):
// request(method, path, type, body) answers with one JSON string, `status`,
// `media` and `body` as base64, or `error` where the kernel carried nothing.
// Synchronous, and carried to the page's own origin, which the engine can
// only reach through the app's own proxy.
(() => {
  const ORIGIN = window.origin;
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE'];
  const fail = error => JSON.stringify({ error });
  const b64 = s => {
    let bin = '';
    for (let i = 0; i < s.length; i++) bin += String.fromCharCode(s.charCodeAt(i) & 0xff);
    return btoa(bin);
  };
  const request = (method, path, type, body) => {
    const m = String(method).toUpperCase();
    if (!METHODS.includes(m)) return fail('a method this frame does not carry: ' + method);
    const p = String(path);
    if (!p.startsWith('/') || p.startsWith('/.rhtn/'))
      return fail('a path is rooted on this resource and not under /.rhtn/');
    const x = new XMLHttpRequest();
    x.open(m, ORIGIN + p, false);
    x.overrideMimeType('text/plain; charset=x-user-defined');
    if (body != null && type != null) x.setRequestHeader('content-type', String(type));
    try { x.send(body == null ? null : String(body)); }
    catch (e) { return fail('the kernel did not carry the request'); }
    const carried = x.getResponseHeader('rhtn-carried');
    if (carried) return fail(decodeURIComponent(carried));
    const media = x.getResponseHeader('rhtn-undeclared') ? null
      : (x.getResponseHeader('content-type') || '').split(';')[0].trim().toLowerCase() || null;
    return JSON.stringify({ status: x.status, media, body: b64(x.responseText) });
  };
  // the object and its descriptor are made in the page's own world, so
  // the page reads them as its own; the function is this script's, lent
  const page = window.wrappedJSObject;
  const rhtn = new page.Object();
  exportFunction(request, rhtn, { defineAs: 'request' });
  page.Object.freeze(rhtn);
  const d = new page.Object();
  d.value = rhtn;
  d.writable = false;
  d.configurable = false;
  page.Object.defineProperty(page, 'rhtn', d);
})();
