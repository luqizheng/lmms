package com.coder.pm.infra.persistence.entity;

import com.baomidou.mybatisplus.annotation.IdType;
import com.baomidou.mybatisplus.annotation.TableField;
import com.baomidou.mybatisplus.annotation.TableId;
import com.baomidou.mybatisplus.annotation.TableName;

import java.time.LocalDateTime;

/**
 * PM acceptance criterion. Maps to {@code pm_acceptance_criteria}.
 *
 * <p>One AC = one {@code Given / When / Then} triple, with a {@code checked} boolean
 * indicating whether it's been validated against the implementation.
 */
@TableName("pm_acceptance_criteria")
public class AcceptanceCriteria {

    @TableId(type = IdType.AUTO)
    private Long id;

    @TableField("user_story_id")
    private Long userStoryId;

    @TableField("given_text")
    private String givenText;

    @TableField("when_text")
    private String whenText;

    @TableField("then_text")
    private String thenText;

    @TableField("checked")
    private Boolean checked;

    @TableField("sort_order")
    private Integer sortOrder;

    @TableField("created_at")
    private LocalDateTime createdAt;

    @TableField("updated_at")
    private LocalDateTime updatedAt;

    public Long getId() { return id; }
    public void setId(Long id) { this.id = id; }

    public Long getUserStoryId() { return userStoryId; }
    public void setUserStoryId(Long userStoryId) { this.userStoryId = userStoryId; }

    public String getGivenText() { return givenText; }
    public void setGivenText(String givenText) { this.givenText = givenText; }

    public String getWhenText() { return whenText; }
    public void setWhenText(String whenText) { this.whenText = whenText; }

    public String getThenText() { return thenText; }
    public void setThenText(String thenText) { this.thenText = thenText; }

    public Boolean getChecked() { return checked; }
    public void setChecked(Boolean checked) { this.checked = checked; }

    public Integer getSortOrder() { return sortOrder; }
    public void setSortOrder(Integer sortOrder) { this.sortOrder = sortOrder; }

    public LocalDateTime getCreatedAt() { return createdAt; }
    public void setCreatedAt(LocalDateTime createdAt) { this.createdAt = createdAt; }

    public LocalDateTime getUpdatedAt() { return updatedAt; }
    public void setUpdatedAt(LocalDateTime updatedAt) { this.updatedAt = updatedAt; }
}
