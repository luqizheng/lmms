package io.digua.model;

import com.fasterxml.jackson.annotation.JsonInclude;

import java.time.Instant;
import java.util.List;

/**
 * Request body for {@code POST /v1/notification/announcements}.
 */
@JsonInclude(JsonInclude.Include.NON_NULL)
public class AnnouncementRequest {
    private String title;
    private String content;
    private String audience;
    private String level;
    private boolean pinned;
    private List<String> channels;
    private Instant expiresAt;

    public AnnouncementRequest() {}

    public static Builder builder() { return new Builder(); }

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

    public List<String> getChannels() { return channels; }
    public void setChannels(List<String> channels) { this.channels = channels; }

    public Instant getExpiresAt() { return expiresAt; }
    public void setExpiresAt(Instant expiresAt) { this.expiresAt = expiresAt; }

    public static final class Builder {
        private final AnnouncementRequest r = new AnnouncementRequest();

        public Builder title(String v) { r.title = v; return this; }
        public Builder content(String v) { r.content = v; return this; }
        public Builder audience(String v) { r.audience = v; return this; }
        public Builder level(String v) { r.level = v; return this; }
        public Builder pinned(boolean v) { r.pinned = v; return this; }
        public Builder channels(List<String> v) { r.channels = v; return this; }
        public Builder expiresAt(Instant v) { r.expiresAt = v; return this; }

        public AnnouncementRequest build() { return r; }
    }
}
