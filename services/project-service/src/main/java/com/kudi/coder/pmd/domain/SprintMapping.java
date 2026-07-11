package com.kudi.coder.pmd.domain;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.GeneratedValue;
import jakarta.persistence.GenerationType;
import jakarta.persistence.Id;
import jakarta.persistence.Table;

import java.time.LocalDate;
import java.time.LocalDateTime;

@Entity
@Table(name = "pmd_sprint_mappings")
public class SprintMapping {

    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    @Column(name = "id")
    private Long id;

    @Column(name = "tenant_id", nullable = false)
    private Long tenantId;

    @Column(name = "project_id", nullable = false)
    private Long projectId;

    @Column(name = "sprint_name", nullable = false, length = 64)
    private String sprintName;

    @Column(name = "starts_on", nullable = false)
    private LocalDate startsOn;

    @Column(name = "ends_on", nullable = false)
    private LocalDate endsOn;

    @Column(name = "actor_id", length = 64)
    private String actorId;

    @Column(name = "created_at", nullable = false, updatable = false)
    private LocalDateTime createdAt;

    @Column(name = "updated_at", nullable = false)
    private LocalDateTime updatedAt;

    public Long getId() { return id; }
    public Long getTenantId() { return tenantId; }
    public Long getProjectId() { return projectId; }
    public String getSprintName() { return sprintName; }
    public LocalDate getStartsOn() { return startsOn; }
    public LocalDate getEndsOn() { return endsOn; }
    public String getActorId() { return actorId; }
    public LocalDateTime getCreatedAt() { return createdAt; }
    public LocalDateTime getUpdatedAt() { return updatedAt; }

    public void setId(Long id) { this.id = id; }
    public void setTenantId(Long tenantId) { this.tenantId = tenantId; }
    public void setProjectId(Long projectId) { this.projectId = projectId; }
    public void setSprintName(String sprintName) { this.sprintName = sprintName; }
    public void setStartsOn(LocalDate startsOn) { this.startsOn = startsOn; }
    public void setEndsOn(LocalDate endsOn) { this.endsOn = endsOn; }
    public void setActorId(String actorId) { this.actorId = actorId; }
}
