package com.kudi.coder.configsdk.model;

import com.fasterxml.jackson.annotation.JsonInclude;

/**
 * Body for {@code POST /v1/config/namespaces}.
 */
@JsonInclude(JsonInclude.Include.NON_NULL)
public class NamespaceRequest {
    private String app;
    private String name;
    private String description;

    public NamespaceRequest() {}

    public static Builder builder() { return new Builder(); }

    public String getApp() { return app; }
    public void setApp(String app) { this.app = app; }

    public String getName() { return name; }
    public void setName(String name) { this.name = name; }

    public String getDescription() { return description; }
    public void setDescription(String description) { this.description = description; }

    public static final class Builder {
        private final NamespaceRequest r = new NamespaceRequest();
        public Builder app(String v) { r.app = v; return this; }
        public Builder name(String v) { r.name = v; return this; }
        public Builder description(String v) { r.description = v; return this; }
        public NamespaceRequest build() { return r; }
    }
}
