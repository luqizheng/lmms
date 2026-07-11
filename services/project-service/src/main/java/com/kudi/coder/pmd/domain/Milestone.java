package com.kudi.coder.pmd.domain;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.EnumType;
import jakarta.persistence.Enumerated;
import jakarta.persistence.GeneratedValue;
import jakarta.persistence.GenerationType;
import jakarta.persistence.Id;
import jakarta.persistence.Table;

import java.time.LocalDate;
import java.time.LocalDateTime;

@Entity
@Table(name = "pmd_milestones")
public class Milestone {

    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    @Column(name = "id")
    private Long id;

    @Column(name = "tenant_id", nullable = false)
    private Long tenantId;

    @Column(name = "project_id", nullable = false)
    private Long projectId;

    @Column(name = "code", nullable = false, length = 64)
    private String code;

    @Column(name = "name", nullable = false, length = 255)
    private String name;

    @Column(name = "due_date")
    private LocalDate dueDate;

    @Enumerated(EnumType.STRING)
    @Column(name = "status", nullable = false, length = 16)
    private MilestoneStatus status = MilestoneStatus.pending;

    @Column(name = "actor_id", length = 64)
    private String actorId;

    @Column(name = "created_at", nullable = false, updatable = false)
    private LocalDateTime createdAt;

    @Column(name = "updated_at", nullable = false)
    private LocalDateTime updatedAt;

    public enum MilestoneStatus { pending, reached, missed, cancelled }

    public Long getId() { return id; }
    public Long getTenantId() { return tenantId; }
    public Long getProjectId() { return projectId; }
    public String getCode() { return code; }
    public String getName() { return name; }
    public LocalDate getDueDate() { return dueDate; }
    public MilestoneStatus getStatus() { return status; }
    public String getActorId() { return actorId; }
    public LocalDateTime getCreatedAt() { return createdAt; }
    public LocalDateTime getUpdatedAt() { return updatedAt; }

    public void setId(Long id) { this.id = id; }
    public void setTenantId(Long tenantId) { this.tenantId = tenantId; }
    public void setProjectId(Long projectId) { this.projectId = projectId; }
    public void setCode(String code) { this.code = code; }
    public void setName(String name) { this.name = name; }
    public void setDueDate(LocalDate dueDate) { this.dueDate = dueDate; }
    public void setStatus(MilestoneStatus status) { this.status = status; }
    public void setActorId(String actorId) { this.actorId = actorId; }
}
