package com.kudi.coder.pmd.domain;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.EnumType;
import jakarta.persistence.Enumerated;
import jakarta.persistence.GeneratedValue;
import jakarta.persistence.GenerationType;
import jakarta.persistence.Id;
import jakarta.persistence.Table;
import org.hibernate.annotations.CreationTimestamp;
import org.hibernate.annotations.UpdateTimestamp;

import java.math.BigDecimal;
import java.time.LocalDateTime;

@Entity
@Table(name = "pmd_tasks")
public class Task {

    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    @Column(name = "id")
    private Long id;

    @Column(name = "tenant_id", nullable = false)
    private Long tenantId;

    @Column(name = "project_id", nullable = false)
    private Long projectId;

    @Column(name = "milestone_id")
    private Long milestoneId;

    @Column(name = "sprint_id")
    private Long sprintId;

    @Column(name = "code", nullable = false, length = 64)
    private String code;

    @Column(name = "title", nullable = false, length = 255)
    private String title;

    @Column(name = "description", columnDefinition = "TEXT")
    private String description;

    @Column(name = "assignee_id", length = 64)
    private String assigneeId;

    @Enumerated(EnumType.STRING)
    @Column(name = "status", nullable = false, length = 16)
    private TaskStatus status = TaskStatus.Backlog;

    @Column(name = "estimate_hours", precision = 6, scale = 2)
    private BigDecimal estimateHours;

    @Column(name = "actor_id", length = 64)
    private String actorId;

    @CreationTimestamp
    @Column(name = "created_at", nullable = false, updatable = false)
    private LocalDateTime createdAt;

    @UpdateTimestamp
    @Column(name = "updated_at", nullable = false)
    private LocalDateTime updatedAt;

    public Long getId() { return id; }
    public Long getTenantId() { return tenantId; }
    public Long getProjectId() { return projectId; }
    public Long getMilestoneId() { return milestoneId; }
    public Long getSprintId() { return sprintId; }
    public String getCode() { return code; }
    public String getTitle() { return title; }
    public String getDescription() { return description; }
    public String getAssigneeId() { return assigneeId; }
    public TaskStatus getStatus() { return status; }
    public BigDecimal getEstimateHours() { return estimateHours; }
    public String getActorId() { return actorId; }
    public LocalDateTime getCreatedAt() { return createdAt; }
    public LocalDateTime getUpdatedAt() { return updatedAt; }

    public void setId(Long id) { this.id = id; }
    public void setTenantId(Long tenantId) { this.tenantId = tenantId; }
    public void setProjectId(Long projectId) { this.projectId = projectId; }
    public void setMilestoneId(Long milestoneId) { this.milestoneId = milestoneId; }
    public void setSprintId(Long sprintId) { this.sprintId = sprintId; }
    public void setCode(String code) { this.code = code; }
    public void setTitle(String title) { this.title = title; }
    public void setDescription(String description) { this.description = description; }
    public void setAssigneeId(String assigneeId) { this.assigneeId = assigneeId; }
    public void setStatus(TaskStatus status) { this.status = status; }
    public void setEstimateHours(BigDecimal estimateHours) { this.estimateHours = estimateHours; }
    public void setActorId(String actorId) { this.actorId = actorId; }
}
