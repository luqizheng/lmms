package com.coder.pm.infra.persistence.entity;

import com.baomidou.mybatisplus.annotation.IdType;
import com.baomidou.mybatisplus.annotation.TableField;
import com.baomidou.mybatisplus.annotation.TableId;
import com.baomidou.mybatisplus.annotation.TableName;

import java.time.LocalDateTime;

/**
 * PM user story. Maps to {@code pm_user_stories}. Children ({@link AcceptanceCriteria})
 * live under a story; sibling order via {@code sortOrder}.
 */
@TableName("pm_user_stories")
public class UserStory {

    @TableId(type = IdType.AUTO)
    private Long id;

    @TableField("requirement_id")
    private Long requirementId;

    @TableField("as_a")
    private String asA;

    @TableField("i_want")
    private String iWant;

    @TableField("so_that")
    private String soThat;

    @TableField("story_points")
    private Integer storyPoints;

    @TableField("status")
    private String status;

    @TableField("sort_order")
    private Integer sortOrder;

    @TableField("created_by")
    private Long createdBy;

    @TableField("created_at")
    private LocalDateTime createdAt;

    @TableField("updated_at")
    private LocalDateTime updatedAt;

    public Long getId() { return id; }
    public void setId(Long id) { this.id = id; }

    public Long getRequirementId() { return requirementId; }
    public void setRequirementId(Long requirementId) { this.requirementId = requirementId; }

    public String getAsA() { return asA; }
    public void setAsA(String asA) { this.asA = asA; }

    public String getIWant() { return iWant; }
    public void setIWant(String iWant) { this.iWant = iWant; }

    public String getSoThat() { return soThat; }
    public void setSoThat(String soThat) { this.soThat = soThat; }

    public Integer getStoryPoints() { return storyPoints; }
    public void setStoryPoints(Integer storyPoints) { this.storyPoints = storyPoints; }

    public String getStatus() { return status; }
    public void setStatus(String status) { this.status = status; }

    public Integer getSortOrder() { return sortOrder; }
    public void setSortOrder(Integer sortOrder) { this.sortOrder = sortOrder; }

    public Long getCreatedBy() { return createdBy; }
    public void setCreatedBy(Long createdBy) { this.createdBy = createdBy; }

    public LocalDateTime getCreatedAt() { return createdAt; }
    public void setCreatedAt(LocalDateTime createdAt) { this.createdAt = createdAt; }

    public LocalDateTime getUpdatedAt() { return updatedAt; }
    public void setUpdatedAt(LocalDateTime updatedAt) { this.updatedAt = updatedAt; }
}
