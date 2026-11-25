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
        String mode = getIntent().getStringExtra("mode");
        if (mode == null) mode = getIntent().getBooleanExtra("walled", false) ? "walled" : "open";
        boolean walled = !mode.equals("open");
        listen(7462);
        String upstream = getIntent().getStringExtra("upstream");
        if (upstream == null) upstream = "192.168.0.11:7461";
        try {
            int port = carrier(upstream);
            File cfg = new File(getFilesDir(), "gecko.yaml");
            try (FileWriter w = new FileWriter(cfg)) {
                String prefs = walled ? walled(port) : "prefs:\n  browser.dom.window.dump.enabled: false\n";
                // peer connections left on, and held to this device's own
                // loopback: the only interface their sockets are made on
                if (mode.equals("live")) {
                    prefs = prefs.replace("media.peerconnection.enabled: false", "media.peerconnection.enabled: true")
                        + "  media.peerconnection.ice.force_interface: \"lo\"\n"
                        + "  media.peerconnection.ice.loopback: true\n"
                        + "  media.peerconnection.ice.obfuscate_host_addresses: false\n";
                }
                // peer connections left on and told to reach peers only
                // through the proxy, which is this app's own and says what
                // it was asked for
                if (mode.equals("caught")) {
                    int proxy = proxy(getIntent().getStringExtra("permit"));
                    prefs = prefs.replace("media.peerconnection.enabled: false", "media.peerconnection.enabled: true")
                        .replace("network.proxy.http_port: 1\n", "network.proxy.http_port: " + proxy + "\n")
                        .replace("network.proxy.ssl_port: 1\n", "network.proxy.ssl_port: " + proxy + "\n")
                        .replace("network.proxy.socks_port: 1\n", "network.proxy.socks_port: " + socks(getIntent().getStringExtra("permit")) + "\n")
                        + "  media.peerconnection.ice.proxy_only: true\n"
                        + "env:\n  MOZ_LOG: \"WebrtcTCPSocket:5,mtransport:4,signaling:3\"\n";
                }
                w.write(prefs);
            }
            Log.i(TAG, "mode=" + mode + " carrier=127.0.0.1:" + port + " upstream=" + upstream);
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

    /**
     * A proxy on the loopback that says what it is asked to open, opens
     * `permit` (`host:port`) and refuses everything else.
     */
    static int proxy(final String permit) throws Exception {
        final ServerSocket server = new ServerSocket(0, 64, InetAddress.getByName("127.0.0.1"));
        new Thread(() -> {
            while (true) {
                try {
                    final Socket in = server.accept();
                    new Thread(() -> {
                        try {
                            InputStream i = in.getInputStream();
                            StringBuilder head = new StringBuilder();
                            int c;
                            while (head.length() < 4096 && (c = i.read()) >= 0) {
                                head.append((char) c);
                                if (head.length() >= 4 && head.substring(head.length() - 4).equals("\r\n\r\n")) break;
                            }
                            String first = head.toString().split("\r\n")[0];
                            String[] words = first.split(" ");
                            boolean open = words.length >= 2 && words[0].equals("CONNECT") && words[1].equals(permit);
                            Log.i(TAG, "proxy asked: " + first + (open ? " -> opened" : " -> refused"));
                            if (!open) {
                                in.getOutputStream().write("HTTP/1.1 403 Forbidden\r\ncontent-length: 0\r\n\r\n".getBytes());
                                in.close();
                                return;
                            }
                            String host = permit.substring(0, permit.lastIndexOf(':'));
                            int to = Integer.parseInt(permit.substring(permit.lastIndexOf(':') + 1));
                            try (Socket out = new Socket(host, to)) {
                                in.getOutputStream().write("HTTP/1.1 200 Connection established\r\n\r\n".getBytes());
                                Thread back = new Thread(() -> pump(out, in));
                                back.start();
                                pump(in, out);
                                back.join();
                            }
                        } catch (Exception e) {
                            Log.w(TAG, "proxy: " + e);
                        } finally {
                            try { in.close(); } catch (Exception e) { }
                        }
                    }).start();
                } catch (Exception e) {
                    return;
                }
            }
        }).start();
        return server.getLocalPort();
    }

    /** The same for a SOCKS5 proxy: says the address asked, opens `permit` alone. */
    static int socks(final String permit) throws Exception {
        final ServerSocket server = new ServerSocket(0, 64, InetAddress.getByName("127.0.0.1"));
        new Thread(() -> {
            while (true) {
                try {
                    final Socket in = server.accept();
                    new Thread(() -> {
                        try {
                            java.io.DataInputStream i = new java.io.DataInputStream(in.getInputStream());
                            OutputStream o = in.getOutputStream();
                            int ver = i.readUnsignedByte();
                            int methods = i.readUnsignedByte();
                            i.readFully(new byte[methods]);
                            o.write(new byte[] {5, 0});
                            i.readUnsignedByte();
                            int cmd = i.readUnsignedByte();
                            i.readUnsignedByte();
                            int atyp = i.readUnsignedByte();
                            String host;
                            if (atyp == 1) {
                                byte[] a = new byte[4];
                                i.readFully(a);
                                host = InetAddress.getByAddress(a).getHostAddress();
                            } else if (atyp == 3) {
                                byte[] a = new byte[i.readUnsignedByte()];
                                i.readFully(a);
                                host = new String(a);
                            } else {
                                byte[] a = new byte[16];
                                i.readFully(a);
                                host = InetAddress.getByAddress(a).getHostAddress();
                            }
                            int port = i.readUnsignedShort();
                            String asked = host + ":" + port;
                            boolean open = cmd == 1 && asked.equals(permit);
                            Log.i(TAG, "socks" + ver + " asked: " + asked + (open ? " -> opened" : " -> refused"));
                            if (!open) {
                                o.write(new byte[] {5, 2, 0, 1, 0, 0, 0, 0, 0, 0});
                                in.close();
                                return;
                            }
                            try (Socket out = new Socket(host, port)) {
                                o.write(new byte[] {5, 0, 0, 1, 0, 0, 0, 0, 0, 0});
                                Thread back = new Thread(() -> pump(out, in));
                                back.start();
                                pump(in, out);
                                back.join();
                            }
                        } catch (Exception e) {
                            Log.w(TAG, "socks: " + e);
                        } finally {
                            try { in.close(); } catch (Exception e) { }
                        }
                    }).start();
                } catch (Exception e) {
                    return;
                }
            }
        }).start();
        return server.getLocalPort();
    }

    /** Say what datagrams arrive on this device's own loopback at `port`. */
    static void listen(final int port) {
        new Thread(() -> {
            try (java.net.DatagramSocket s = new java.net.DatagramSocket(port, InetAddress.getByName("127.0.0.1"))) {
                Log.i(TAG, "datagrams bound 127.0.0.1:" + port);
                byte[] buf = new byte[2048];
                for (int n = 1; n <= 64; n++) {
                    java.net.DatagramPacket p = new java.net.DatagramPacket(buf, buf.length);
                    s.receive(p);
                    boolean stun = p.getLength() >= 20 && buf[0] == 0 && buf[1] == 1
                        && buf[4] == 0x21 && buf[5] == 0x12 && (buf[6] & 0xff) == 0xa4 && buf[7] == 0x42;
                    Log.i(TAG, "datagram " + n + " from " + p.getSocketAddress() + " bytes " + p.getLength() + " stun " + stun);
                }
            } catch (Exception e) {
                Log.w(TAG, "datagrams: " + e);
            }
        }).start();
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
