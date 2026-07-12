package com.coder.pm.infra.sdk;

import com.kudi.coder.auditsdk.AuditServiceClient;
import com.kudi.coder.auditsdk.model.AuditEvent;
import com.kudi.coder.auditsdk.model.AuditLog;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;
import org.springframework.security.authentication.UsernamePasswordAuthenticationToken;
import org.springframework.security.core.authority.SimpleGrantedAuthority;
import org.springframework.security.core.context.SecurityContextHolder;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Pure unit tests for {@link AuditEventReporter}.
 *
 * <p>Stubs {@link AuditServiceClient} via anonymous subclass — no Spring context, no Mockito,
 * no SDK HTTP traffic. The base URL is intentionally {@code http://invalid} because we
 * override {@code sendEvent} and never let the request go out.
 */
class AuditEventReporterTest {

    @AfterEach
    void clearSecurityContext() {
        SecurityContextHolder.clearContext();
    }

    private static AuditServiceClient stubCapturing(List<AuditEvent> sink) {
        return new AuditServiceClient("http://invalid", () -> "t") {
            @Override
            public AuditLog sendEvent(AuditEvent event) {
                sink.add(event);
                AuditLog out = new AuditLog();
                out.setId("log-" + sink.size());
                out.setActor(event.getActor());
                out.setAction(event.getAction());
                return out;
            }
        };
    }

    @Test
    void report_calls_client_with_expected_event_payload() {
        AtomicReference<AuditEvent> captured = new AtomicReference<>();
        AuditServiceClient client = new AuditServiceClient("http://invalid", () -> "t") {
            @Override
            public AuditLog sendEvent(AuditEvent event) {
                captured.set(event);
                AuditLog out = new AuditLog();
                out.setId("log-1");
                out.setActor(event.getActor());
                out.setAction(event.getAction());
                return out;
            }
        };
        new AuditEventReporter(client).report(
                "alice", "PM_TEST_ACTION", "pm_test", "42", Map.of("k", "v"));

        AuditEvent event = captured.get();
        assertNotNull(event);
        assertEquals("alice", event.getActor());
        assertEquals("PM_TEST_ACTION", event.getAction());
        assertEquals("pm_test", event.getResourceType());
        assertEquals("42", event.getResourceId());
        assertNotNull(event.getPayload());
        assertEquals("v", event.getPayload().get("k"));
        assertNotNull(event.getOccurredAt());
    }

    @Test
    void report_swallows_runtime_exception_from_client() {
        AuditServiceClient throwingClient = new AuditServiceClient("http://invalid", () -> "t") {
            @Override
            public AuditLog sendEvent(AuditEvent event) {
                throw new RuntimeException("simulated audit-service 500");
            }
        };
        AuditEventReporter reporter = new AuditEventReporter(throwingClient);
        assertDoesNotThrow(() ->
                reporter.report("alice", "PM_TEST", "pm_test", "1", Map.of()));
    }

    @Test
    void report_accepts_null_payload() {
        AtomicReference<AuditEvent> captured = new AtomicReference<>();
        AuditServiceClient client = new AuditServiceClient("http://invalid", () -> "t") {
            @Override
            public AuditLog sendEvent(AuditEvent event) {
                captured.set(event);
                return new AuditLog();
            }
        };
        new AuditEventReporter(client).report("bob", "PM_NULL_PAYLOAD", "x", "1", null);
        assertNotNull(captured.get());
        assertEquals("bob", captured.get().getActor());
    }

    @Test
    void report_passes_through_actor_argument_when_no_security_context() {
        SecurityContextHolder.clearContext();
        List<AuditEvent> sink = new ArrayList<>();
        new AuditEventReporter(stubCapturing(sink))
                .report("ignored", "T", "x", "1", Map.of());
        assertEquals("ignored", sink.get(0).getActor(),
                "Reporter is a pass-through; actor extraction lives at service.currentActor()");
    }

    @Test
    void report_passes_through_actor_argument_when_principal_is_anonymousUser() {
        SecurityContextHolder.getContext().setAuthentication(
                new UsernamePasswordAuthenticationToken(
                        "anonymousUser", "n/a",
                        List.of(new SimpleGrantedAuthority("ROLE_ANONYMOUS"))));
        List<AuditEvent> sink = new ArrayList<>();
        new AuditEventReporter(stubCapturing(sink))
                .report("ignored", "T", "x", "1", Map.of());
        assertEquals("ignored", sink.get(0).getActor(),
                "Spring's default anonymousUser principal must NOT mutate the caller-supplied actor");
    }

    @Test
    void report_passes_through_actor_argument_when_authenticated_as_long() {
        SecurityContextHolder.getContext().setAuthentication(
                new UsernamePasswordAuthenticationToken(
                        42L, "alice", List.of(new SimpleGrantedAuthority("ROLE_USER"))));
        List<AuditEvent> sink = new ArrayList<>();
        new AuditEventReporter(stubCapturing(sink))
                .report("ignored", "T", "x", "1", Map.of());
        assertEquals("ignored", sink.get(0).getActor(),
                "Caller-supplied actor wins; SecurityContext is not consulted by the reporter");
    }

    @Test
    void report_assigns_distinct_occurredAt_to_each_event() throws InterruptedException {
        List<AuditEvent> sink = new ArrayList<>();
        AuditEventReporter reporter = new AuditEventReporter(stubCapturing(sink));
        reporter.report("u", "T1", "x", "1", Map.of());
        Thread.sleep(5);
        reporter.report("u", "T2", "x", "2", Map.of());
        assertEquals(2, sink.size());
        assertNotEquals(sink.get(0).getOccurredAt(), sink.get(1).getOccurredAt(),
                "Each report should generate a fresh timestamp");
        assertFalse(sink.get(0).getOccurredAt().isAfter(sink.get(1).getOccurredAt()),
                "Sequence must be non-decreasing");
        assertTrue(sink.get(1).getOccurredAt().isAfter(sink.get(0).getOccurredAt()),
                "Later reports should have later timestamps");
    }
}
