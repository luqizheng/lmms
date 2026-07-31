package io.digua.web;

import io.digua.AuditServiceClient;
import io.digua.AuditEvent;
import io.digua.AuditLog;
import io.digua.AuditLogQuery;
import io.digua.PageResult;
import io.digua.ApiResponse;
import io.digua.AuditPublisher;
import io.digua.JwtAuthInterceptor;
import io.digua.Principal;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

import javax.servlet.http.HttpServletRequest;
import java.time.Instant;
import java.util.Collections;
import java.util.HashMap;
import java.util.Map;

@RestController
@RequestMapping("/demo/audit")
public class AuditDemoController {

    private final AuditServiceClient auditClient;
    private final AuditPublisher publisher;

    public AuditDemoController(AuditServiceClient auditClient, AuditPublisher publisher) {
        this.auditClient = auditClient;
        this.publisher = publisher;
    }

    @PostMapping("/event")
    public ApiResponse<AuditLog> sendEvent(@RequestBody Map<String, Object> body, HttpServletRequest req) {
        Principal p = JwtAuthInterceptor.current(req);
        String actor = p != null ? p.getUserId() : (String) body.getOrDefault("actor", "anonymous");
        String action = (String) body.get("action");
        String resourceType = (String) body.get("resourceType");
        String resourceId = String.valueOf(body.get("resourceId"));
        @SuppressWarnings("unchecked")
        Map<String, Object> payload = (Map<String, Object>) body.getOrDefault("payload", new HashMap<String, Object>());

        AuditEvent ev = new AuditEvent(actor, action, resourceType, resourceId, payload, Instant.now());
        return ApiResponse.ok(auditClient.sendEvent(ev));
    }

    @GetMapping("/logs")
    public ApiResponse<PageResult<AuditLog>> queryLogs(
            @RequestParam(required = false) String actor,
            @RequestParam(required = false) String action,
            @RequestParam(defaultValue = "1") int page,
            @RequestParam(defaultValue = "20") int size) {
        AuditLogQuery q = new AuditLogQuery();
        q.setActor(actor);
        q.setAction(action);
        q.setPage(page);
        q.setSize(size);
        return ApiResponse.ok(auditClient.queryLogs(q));
    }

    @PostMapping("/event/soft")
    public ApiResponse<Map<String, Object>> sendSoft(@RequestBody Map<String, Object> body, HttpServletRequest req) {
        Principal p = JwtAuthInterceptor.current(req);
        String actor = p != null ? p.getUserId() : "anonymous";
        @SuppressWarnings("unchecked")
        Map<String, Object> payload = (Map<String, Object>) body.getOrDefault("payload", new HashMap<String, Object>());
        publisher.publish(actor,
                (String) body.get("action"),
                (String) body.get("resourceType"),
                String.valueOf(body.get("resourceId")),
                payload);
        return ApiResponse.ok(Collections.<String, Object>singletonMap("accepted", Boolean.TRUE));
    }
}
