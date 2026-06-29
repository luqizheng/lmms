package com.kudi.coder.demowebapp.audit;

import com.kudi.coder.auditsdk.AuditServiceClient;
import com.kudi.coder.auditsdk.model.AuditEvent;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;

import java.time.Instant;
import java.util.Map;

@Component
public class AuditPublisher {

    private static final Logger log = LoggerFactory.getLogger(AuditPublisher.class);

    private final AuditServiceClient client;

    public AuditPublisher(AuditServiceClient client) {
        this.client = client;
    }

    public void publish(String actor, String action, String resourceType,
                        String resourceId, Map<String, Object> payload) {
        AuditEvent ev = new AuditEvent(actor, action, resourceType, resourceId, payload, Instant.now());
        try {
            client.sendEvent(ev);
        } catch (Exception e) {
            log.warn("audit publish failed (action={} resourceType={} resourceId={}): {}",
                    action, resourceType, resourceId, e.getMessage());
        }
    }
}
