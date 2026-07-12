package com.coder.pm.infra.config;

import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Component;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.time.Instant;
import java.util.Map;
import java.util.function.Supplier;

/**
 * Supplies a short-lived service JWT for outbound SDK calls (audit, file,
 * notification, config). The JWT is obtained by exchanging the bootstrap
 * token at {@code POST /tokens/service-jwt} — a standard pattern used by
 * every service in the platform (see services/token-service/src/routes/
 * service_jwt.rs).
 *
 * <p>The cached token is refreshed before expiry (5-minute safety margin).
 * If the token-service is unreachable, falls back to the static
 * {@code pm.sdk.bearer.static-token} (dev-bypass-token in dev profile).
 */
@Component
public class ServiceJwtSupplier implements Supplier<String> {

    private static final Logger log = LoggerFactory.getLogger(ServiceJwtSupplier.class);
    private static final int REFRESH_MARGIN_SEC = 300;
    private static final Duration TIMEOUT = Duration.ofSeconds(5);

    private final HttpClient http;
    private final ObjectMapper mapper;
    private final String tokenServiceUrl;
    private final String bootstrapToken;
    private final String serviceAudience;
    private final int tenantId;
    private final String fallbackToken;

    private volatile String cachedJwt;
    private volatile long expiresAtEpoch;

    public ServiceJwtSupplier(
            @Value("${pm.sdk.bearer.bootstrap-token:}") String bootstrapToken,
            @Value("${pm.sdk.service-jwt.audience:audit-service}") String serviceAudience,
            @Value("${pm.sdk.service-jwt.tenant-id:1}") int tenantId,
            @Value("${pm.sdk.bearer.static-token:dev-bypass-token}") String fallbackToken,
            @Value("${pm.sdk.token-service-url:http://localhost:3000}") String tokenServiceUrl) {
        this.bootstrapToken = bootstrapToken;
        this.serviceAudience = serviceAudience;
        this.tenantId = tenantId;
        this.fallbackToken = fallbackToken;
        this.tokenServiceUrl = tokenServiceUrl;
        this.http = HttpClient.newBuilder()
                .connectTimeout(TIMEOUT)
                .build();
        this.mapper = new ObjectMapper();
    }

    @Override
    public String get() {
        if (bootstrapToken == null || bootstrapToken.isBlank()) {
            log.debug("ServiceJwtSupplier: no bootstrap token configured, using fallback");
            return fallbackToken;
        }
        if (cachedJwt != null && Instant.now().getEpochSecond() < expiresAtEpoch - REFRESH_MARGIN_SEC) {
            return cachedJwt;
        }
        return acquire();
    }

    private synchronized String acquire() {
        if (cachedJwt != null && Instant.now().getEpochSecond() < expiresAtEpoch - REFRESH_MARGIN_SEC) {
            return cachedJwt;
        }
        try {
            String body = mapper.writeValueAsString(Map.of("aud", serviceAudience, "tid", tenantId));
            HttpRequest req = HttpRequest.newBuilder()
                    .uri(URI.create(tokenServiceUrl + "/tokens/service-jwt"))
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer " + bootstrapToken)
                    .POST(HttpRequest.BodyPublishers.ofString(body))
                    .timeout(TIMEOUT)
                    .build();
            HttpResponse<String> resp = http.send(req, HttpResponse.BodyHandlers.ofString());
            if (resp.statusCode() / 100 != 2) {
                log.warn("ServiceJwtSupplier: token-service returned {}", resp.statusCode());
                return fallbackToken;
            }
            @SuppressWarnings("unchecked")
            Map<String, Object> data = mapper.readValue(resp.body(), Map.class);
            cachedJwt = (String) data.get("access_token");
            Object expiresIn = data.get("expires_in");
            expiresAtEpoch = expiresIn instanceof Number n
                    ? Instant.now().getEpochSecond() + n.longValue()
                    : Instant.now().getEpochSecond() + 3600;
            log.info("ServiceJwtSupplier: acquired JWT for aud={}, expires in {}s",
                    serviceAudience, expiresAtEpoch - Instant.now().getEpochSecond());
            return cachedJwt;
        } catch (Exception ex) {
            log.warn("ServiceJwtSupplier: acquire failed: {}", ex.getMessage());
            return fallbackToken;
        }
    }
}
