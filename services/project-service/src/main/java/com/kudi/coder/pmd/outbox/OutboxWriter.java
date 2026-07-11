package com.kudi.coder.pmd.outbox;

import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.kudi.coder.pmd.config.TenantContext;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Propagation;
import org.springframework.transaction.annotation.Transactional;

import java.time.OffsetDateTime;
import java.util.Map;
import java.util.UUID;

/**
 * Writes outbox events to {@code pmd_outbox_events} via {@link JdbcTemplate}
 * native SQL in the same {@code @Transactional} boundary as the business
 * write. **No JPA {@code EntityManager.persist}** — column layout must match
 * {@code libs/common-event::outbox::MySqlOutboxRepo::enqueue} exactly so the
 * Rust {@code pmd-outbox-publisher} poller sees consistent rows.
 *
 * Schema columns (declared in services/project-service/migrations/mysql/V001__init.sql):
 *   event_id, event_type, routing_key, envelope_json, trace_id, tenant_id,
 *   producer, created_at (default), published_at (NULL initially),
 *   attempts (default 0), last_error (NULL initially), id (auto-increment).
 *
 * Caller-side contract:
 * - {@code enqueue(...)} joins the caller's @Transactional method.
 * - {@code enqueueWithLog(...)} additionally writes {@code pmd_llm_refinement_log}
 *   in the same transaction (T7 wiring).
 */
@Service
public class OutboxWriter {

    private final JdbcTemplate jdbc;
    private final ObjectMapper mapper;

    public OutboxWriter(JdbcTemplate jdbc, ObjectMapper mapper) {
        this.jdbc = jdbc;
        this.mapper = mapper;
    }

    /**
     * Insert one outbox row using {@code MANDATORY} propagation: the caller MUST
     * have an active transaction. If a misuse is caught at runtime the
     * {@code UnexpectedRollbackException} bubbles up to the controller advice.
     */
    @Transactional(propagation = Propagation.MANDATORY)
    public void enqueue(String routingKey, Map<String, Object> envelope) {
        String eventId = UUID.randomUUID().toString();
        String envelopeJson = toJson(envelope);
        String traceId = currentTraceId();
        Long tenantId = TenantContext.getTenantIdOrNull();
        String producer = "project-service";

        jdbc.update("""
                INSERT INTO pmd_outbox_events
                  (event_id, event_type, routing_key, envelope_json,
                   trace_id, tenant_id, producer)
                VALUES (?, ?, ?, ?, ?, ?, ?)
                """,
                eventId,
                routingKey,
                routingKey,
                envelopeJson,
                traceId,
                tenantId,
                producer);
    }

    /**
     * Used by T7's {@code LlmRefineController} — same transaction enqueue plus
     * a paired {@code pmd_llm_refinement_log} row (audit-of-audit).
     */
    @Transactional(propagation = Propagation.MANDATORY)
    public void enqueueWithLog(String routingKey,
                               Map<String, Object> envelope,
                               Long projectId,
                               Long taskId,
                               String requestText,
                               String responseText) {
        String caller = envelope.getOrDefault("caller_user_id", "").toString();
        Long tenantId = TenantContext.getTenantIdOrNull();

        jdbc.update("""
                INSERT INTO pmd_llm_refinement_log
                  (tenant_id, project_id, task_id, caller_user_id, request, response)
                VALUES (?, ?, ?, ?, ?, ?)
                """,
                tenantId,
                projectId,
                taskId,
                caller,
                requestText,
                responseText);

        enqueue(routingKey, envelope);
    }

    private String currentTraceId() {
        // The gateway injects X-Trace-Id via common-telemetry-axum; for MVP we
        // simply read from ThreadLocal if a tracing library exposes one, else
        // emit a short UUID suffix. The Rust publisher forwards trace_id to
        // audit_events for correlation.
        try {
            Class<?> tlCls = Class.forName("io.opentelemetry.context.Context");
            Object ctx = tlCls.getMethod("current").invoke(null);
            Object span = Class.forName("io.opentelemetry.trace.Span")
                    .getMethod("fromContext", tlCls).invoke(null, ctx);
            Object spanCtx = span.getClass().getMethod("getSpanContext").invoke(span);
            String traceId = (String) spanCtx.getClass().getMethod("getTraceId").invoke(spanCtx);
            if (traceId != null && !traceId.isBlank()) return traceId;
        } catch (Throwable ignored) {
            // OpenTelemetry not on classpath; fall through.
        }
        return "pmd-" + OffsetDateTime.now().toEpochSecond() + "-"
                + UUID.randomUUID().toString().substring(0, 8);
    }

    private String toJson(Map<String, Object> envelope) {
        try {
            return mapper.writeValueAsString(envelope);
        } catch (JsonProcessingException e) {
            throw new IllegalStateException("outbox envelope not serializable", e);
        }
    }
}
