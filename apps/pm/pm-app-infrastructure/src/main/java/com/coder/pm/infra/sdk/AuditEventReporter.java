package com.coder.pm.infra.sdk;

import com.kudi.coder.auditsdk.AuditServiceClient;
import com.kudi.coder.auditsdk.model.AuditEvent;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;

import java.time.Instant;
import java.util.Map;

/**
 * Thin wrapper around {@link AuditServiceClient} so controllers do not need to deal with
 * the SDK directly. MVP policy: audit failures must not block business operations — we log
 * and move on.
 *
 * <p>Future callers (controllers / services) inject this and call {@link #report} after each
 * write. Specific event names can be defined as constants when downstream consumers care.
 */
@Component
public class AuditEventReporter {

    private static final Logger log = LoggerFactory.getLogger(AuditEventReporter.class);

    private final AuditServiceClient client;

    public AuditEventReporter(AuditServiceClient client) {
        this.client = client;
    }

    public void report(String actor,
                       String action,
                       String resourceType,
                       String resourceId,
                       Map<String, Object> payload) {
        try {
            client.sendEvent(new AuditEvent(
                    actor, action, resourceType, resourceId, payload, Instant.now()));
        } catch (RuntimeException ex) {
            log.warn("audit-service report failed (action={}, resource={}/{}): {}",
                    action, resourceType, resourceId, ex.getMessage());
        }
    }
}
