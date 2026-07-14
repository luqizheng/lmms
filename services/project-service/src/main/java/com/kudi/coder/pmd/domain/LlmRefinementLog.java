package com.kudi.coder.pmd.domain;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.GeneratedValue;
import jakarta.persistence.GenerationType;
import jakarta.persistence.Id;
import jakarta.persistence.Table;
import org.hibernate.annotations.CreationTimestamp;

import java.time.LocalDateTime;

/**
 * Append-only audit log for {@code POST /project/llm/refine} calls. Written
 * by {@code OutboxWriter.enqueueWithLog(...)} in the same {@code @Transactional}
 * boundary as the corresponding {@code pmd_outbox_events} row. Read-only here;
 * never updated through the service layer.
 */
@Entity
@Table(name = "pmd_llm_refinement_log")
public class LlmRefinementLog {

    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    @Column(name = "id")
    private Long id;

    @Column(name = "tenant_id")
    private Long tenantId;

    @Column(name = "project_id")
    private Long projectId;

    @Column(name = "task_id")
    private Long taskId;

    @Column(name = "caller_user_id", nullable = false, length = 64)
    private String callerUserId;

    @Column(name = "request", nullable = false, columnDefinition = "TEXT")
    private String request;

    @Column(name = "response", columnDefinition = "TEXT")
    private String response;

    @CreationTimestamp
    @Column(name = "created_at", nullable = false, updatable = false)
    private LocalDateTime createdAt;

    public Long getId() { return id; }
    public Long getTenantId() { return tenantId; }
    public Long getProjectId() { return projectId; }
    public Long getTaskId() { return taskId; }
    public String getCallerUserId() { return callerUserId; }
    public String getRequest() { return request; }
    public String getResponse() { return response; }
    public LocalDateTime getCreatedAt() { return createdAt; }
}
