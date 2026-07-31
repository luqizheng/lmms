package io.digua.model;

import com.fasterxml.jackson.annotation.JsonInclude;

import java.time.Instant;
import java.util.List;

/**
 * Announcement resource as returned by notification-service.
 *
 * <p>Mirrors the fields exposed by {@code GET /v1/notification/announcements/{id}}.
 */
@JsonInclude(JsonInclude.Include.NON_NULL)
public class Announcement {
    private Long id;
    private String title;
    private String content;
    private String audience;
    private String level;
    private boolean pinned;
    private String status;
    private Long creatorId;
    private Instant publishedAt;
    private Instant expiresAt;
    private List<String> channels;

    public Long getId() { return id; }
    public void setId(Long id) { this.id = id; }

    public String getTitle() { return title; }
    public void setTitle(String title) { this.title = title; }

    public String getContent() { return content; }
    public void setContent(String content) { this.content = content; }

    public String getAudience() { return audience; }
    public void setAudience(String audience) { this.audience = audience; }

    public String getLevel() { return level; }
    public void setLevel(String level) { this.level = level; }

    public boolean isPinned() { return pinned; }
    public void setPinned(boolean pinned) { this.pinned = pinned; }

    public String getStatus() { return status; }
    public void setStatus(String status) { this.status = status; }

    public Long getCreatorId() { return creatorId; }
    public void setCreatorId(Long creatorId) { this.creatorId = creatorId; }

    public Instant getPublishedAt() { return publishedAt; }
    public void setPublishedAt(Instant publishedAt) { this.publishedAt = publishedAt; }

    public Instant getExpiresAt() { return expiresAt; }
    public void setExpiresAt(Instant expiresAt) { this.expiresAt = expiresAt; }

    public List<String> getChannels() { return channels; }
    public void setChannels(List<String> channels) { this.channels = channels; }
}
