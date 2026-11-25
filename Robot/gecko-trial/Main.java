package com.comptus.geckotrial;

import android.app.Activity;
import android.os.Bundle;
import android.util.Log;
import java.io.File;
import java.io.FileWriter;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.util.concurrent.atomic.AtomicInteger;
import org.mozilla.geckoview.GeckoRuntime;
import org.mozilla.geckoview.GeckoRuntimeSettings;
import org.mozilla.geckoview.GeckoSession;
import org.mozilla.geckoview.GeckoView;

/**
 * A throwaway frame on GeckoView: every load of the page goes to a carrier
 * on this device's own loopback, which stands where the kernel would and
 * relays to `upstream`; with `walled`, the engine is told to send
 * everything else nowhere and to offer no peer connection.
 */
public class Main extends Activity {
    static final String TAG = "geckotrial";
    static GeckoRuntime runtime;
    static final AtomicInteger carried = new AtomicInteger();

    @Override
    protected void onCreate(Bundle saved) {
        super.onCreate(saved);
        boolean walled = getIntent().getBooleanExtra("walled", false);
        String upstream = getIntent().getStringExtra("upstream");
        if (upstream == null) upstream = "192.168.0.11:7461";
        try {
            int port = carrier(upstream);
            File cfg = new File(getFilesDir(), "gecko.yaml");
            try (FileWriter w = new FileWriter(cfg)) {
                w.write(walled ? walled(port) : "prefs:\n  browser.dom.window.dump.enabled: false\n");
            }
            Log.i(TAG, "walled=" + walled + " carrier=127.0.0.1:" + port + " upstream=" + upstream);
            GeckoRuntimeSettings settings = new GeckoRuntimeSettings.Builder()
                .configFilePath(cfg.getAbsolutePath())
                .consoleOutput(true)
                .build();
            if (runtime == null) runtime = GeckoRuntime.create(getApplicationContext(), settings);
            GeckoSession session = new GeckoSession();
            session.open(runtime);
            GeckoView view = new GeckoView(this);
            view.setSession(session);
            setContentView(view);
            session.loadUri("http://127.0.0.1:" + port + "/");
        } catch (Exception e) {
            Log.e(TAG, "failed", e);
        }
    }

    static String walled(int port) {
        return "prefs:\n"
            + "  media.peerconnection.enabled: false\n"
            + "  network.proxy.type: 1\n"
            + "  network.proxy.http: \"127.0.0.1\"\n"
            + "  network.proxy.http_port: 1\n"
            + "  network.proxy.ssl: \"127.0.0.1\"\n"
            + "  network.proxy.ssl_port: 1\n"
            + "  network.proxy.socks: \"127.0.0.1\"\n"
            + "  network.proxy.socks_port: 1\n"
            + "  network.proxy.socks_remote_dns: true\n"
            + "  network.proxy.allow_hijacking_localhost: true\n"
            + "  network.proxy.failover_direct: false\n"
            + "  network.proxy.no_proxies_on: \"127.0.0.1:" + port + "\"\n"
            + "  network.webtransport.enabled: false\n"
            + "  network.http.http3.enable: false\n"
            + "  network.dns.disablePrefetch: true\n"
            + "  network.dns.disablePrefetchFromHTTPS: true\n"
            + "  network.prefetch-next: false\n"
            + "  network.http.speculative-parallel-limit: 0\n"
            + "  network.predictor.enabled: false\n"
            + "  network.captive-portal-service.enabled: false\n"
            + "  network.connectivity-service.enabled: false\n";
    }

    /** A relay on the loopback: each connection is piped to `upstream`. */
    static int carrier(String upstream) throws Exception {
        final String host = upstream.substring(0, upstream.lastIndexOf(':'));
        final int to = Integer.parseInt(upstream.substring(upstream.lastIndexOf(':') + 1));
        final ServerSocket server = new ServerSocket(0, 64, InetAddress.getByName("127.0.0.1"));
        new Thread(() -> {
            while (true) {
                try {
                    final Socket in = server.accept();
                    final int n = carried.incrementAndGet();
                    if (n <= 5 || n % 10 == 0) Log.i(TAG, "carried " + n);
                    new Thread(() -> {
                        try (Socket out = new Socket(host, to)) {
                            Thread back = new Thread(() -> pump(out, in));
                            back.start();
                            pump(in, out);
                            back.join();
                        } catch (Exception e) {
                            Log.w(TAG, "carry: " + e);
                        } finally {
                            try { in.close(); } catch (Exception e) { }
                        }
                    }).start();
                } catch (Exception e) {
                    Log.w(TAG, "accept: " + e);
                    return;
                }
            }
        }).start();
        return server.getLocalPort();
    }

    static void pump(Socket from, Socket to) {
        try {
            InputStream i = from.getInputStream();
            OutputStream o = to.getOutputStream();
            byte[] buf = new byte[65536];
            int n;
            while ((n = i.read(buf)) > 0) { o.write(buf, 0, n); o.flush(); }
        } catch (Exception e) {
        } finally {
            try { to.shutdownOutput(); } catch (Exception e) { }
        }
    }
}
