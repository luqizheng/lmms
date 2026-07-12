package com.coder.pm.web;

import jakarta.servlet.http.HttpServletRequest;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.http.ResponseEntity;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.time.Instant;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * Liveness + readiness.
 *
 * <ul>
 *   <li>{@code GET /pm-api/v1/health} — liveness. Returns 200 + JSON summary.
 *       <strong>Does not</strong> touch DB so it can succeed during startup or DB outage.</li>
 *   <li>{@code GET /pm-api/v1/health/ready} — readiness. Pings MySQL. 200 if OK,
 *       503 if DB unreachable.</li>
 * </ul>
 *
 * <p>Used by Kubernetes / Docker {@code HEALTHCHECK}.
 */
@RestController
@RequestMapping("/v1/health")
public class HealthController {

    private static final Logger log = LoggerFactory.getLogger(HealthController.class);
    private static final String SERVICE = "pm-platform";
    private static final String VERSION = "0.1.0";

    private final JdbcTemplate jdbcTemplate;

    @Autowired
    public HealthController(JdbcTemplate jdbcTemplate) {
        this.jdbcTemplate = jdbcTemplate;
    }

    @GetMapping(produces = MediaType.APPLICATION_JSON_VALUE)
    public ResponseEntity<Map<String, Object>> liveness() {
        Map<String, Object> body = baseBody("UP", "UP", "UP");
        return ResponseEntity.ok(body);
    }

    @GetMapping(value = "/ready", produces = MediaType.APPLICATION_JSON_VALUE)
    public ResponseEntity<Map<String, Object>> readiness() {
        String dbStatus;
        try {
            Integer one = jdbcTemplate.queryForObject("SELECT 1", Integer.class);
            dbStatus = one != null && one == 1 ? "UP" : "DOWN";
        } catch (Exception ex) {
            log.warn("readiness: DB ping failed: {}", ex.getMessage());
            dbStatus = "DOWN";
        }
        Map<String, Object> body = baseBody(dbStatus, dbStatus, "UP");
        return "UP".equals(dbStatus)
                ? ResponseEntity.ok(body)
                : ResponseEntity.status(503).body(body);
    }

    private Map<String, Object> baseBody(String status, String db, String deps) {
        Map<String, Object> body = new LinkedHashMap<>();
        body.put("status", status);
        body.put("db", db);
        body.put("dependencies", deps);
        body.put("service", SERVICE);
        body.put("version", VERSION);
        body.put("timestamp", Instant.now().toString());
        return body;
    }

    /** Unused; kept for diagnostics. */
    @SuppressWarnings("unused")
    private String clientIp(HttpServletRequest req) {
        String fwd = req.getHeader("X-Forwarded-For");
        return fwd != null ? fwd.split(",")[0].trim() : req.getRemoteAddr();
    }
}
