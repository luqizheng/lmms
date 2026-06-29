package com.kudi.coder.demowebapp.config;

import com.kudi.coder.httpclient.KudiHttpClient;
import com.kudi.coder.httpclient.HttpTransport;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Component;

import javax.annotation.PostConstruct;
import javax.annotation.PreDestroy;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.locks.Condition;
import java.util.concurrent.locks.ReentrantLock;

@Component
public class ConfigPoller {

    private static final Logger log = LoggerFactory.getLogger(ConfigPoller.class);

    @Autowired
    private KudiHttpClient kudiHttpClient;

    @Autowired
    private KudiCoderProperties properties;

    private final ConcurrentHashMap<String, String> cache = new ConcurrentHashMap<>();
    private final ConcurrentHashMap<String, ReentrantLock> keyLocks = new ConcurrentHashMap<>();
    private final ConcurrentHashMap<String, Condition> keyConds = new ConcurrentHashMap<>();
    private ScheduledExecutorService scheduler;

    @PostConstruct
    public void start() {
        scheduler = Executors.newSingleThreadScheduledExecutor(r -> {
            Thread t = new Thread(r, "config-poller");
            t.setDaemon(true);
            return t;
        });
        scheduler.scheduleAtFixedRate(this::poll, 0, 5, TimeUnit.SECONDS);
    }

    @PreDestroy
    public void stop() {
        if (scheduler != null) scheduler.shutdownNow();
    }

    public String get(String key) {
        return cache.get(key);
    }

    public boolean waitForChange(String key, String expected, long timeoutSec) throws InterruptedException {
        ReentrantLock lock = keyLocks.computeIfAbsent(key, k -> new ReentrantLock());
        Condition cond = keyConds.computeIfAbsent(key, k -> lock.newCondition());
        long deadline = System.currentTimeMillis() + timeoutSec * 1000L;
        lock.lock();
        try {
            while (true) {
                String current = cache.get(key);
                if (expected == null ? current == null : !expected.equals(current)) {
                    return true;
                }
                long remaining = deadline - System.currentTimeMillis();
                if (remaining <= 0) return false;
                cond.await(remaining, TimeUnit.MILLISECONDS);
            }
        } finally {
            lock.unlock();
        }
    }

    private void poll() {
        try {
            String url = properties.getGateway().getBaseUrl() + "/v1/cfg/namespaces/demo/values";
            HttpTransport.Response resp = kudiHttpClient.get(url, null);
            if (resp != null && resp.body != null && resp.body.startsWith("{")) {
                parseFlatJson(resp.body);
            }
        } catch (Exception e) {
            log.debug("config poll failed: {}", e.getMessage());
        }
    }

    private void parseFlatJson(String body) {
        String inner = body.substring(1, body.length() - 1).trim();
        if (inner.isEmpty()) return;
        for (String pair : inner.split(",")) {
            int colon = pair.indexOf(':');
            if (colon < 0) continue;
            String k = pair.substring(0, colon).trim().replaceAll("^\"|\"$", "");
            String v = pair.substring(colon + 1).trim().replaceAll("^\"|\"$", "");
            String old = cache.put(k, v);
            if (old != null && !old.equals(v)) {
                log.info("config changed: {} = {} (was {})", k, v, old);
                ReentrantLock l = keyLocks.get(k);
                if (l != null) {
                    Condition c = keyConds.get(k);
                    if (c != null) {
                        l.lock();
                        try { c.signalAll(); } finally { l.unlock(); }
                    }
                }
            }
        }
    }
}
