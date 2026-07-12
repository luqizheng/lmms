package com.kudi.coder.configsdk.model;

import com.fasterxml.jackson.annotation.JsonInclude;

/**
 * Body for {@code POST /v1/config/namespaces/{id}/items}.
 */
@JsonInclude(JsonInclude.Include.NON_NULL)
public class ConfigItemRequest {
    private String key;
    private String value;
    private String valueType;
    private String description;

    public ConfigItemRequest() {}

    public static Builder builder() { return new Builder(); }

    public String getKey() { return key; }
    public void setKey(String key) { this.key = key; }

    public String getValue() { return value; }
    public void setValue(String value) { this.value = value; }

    public String getValueType() { return valueType; }
    public void setValueType(String valueType) { this.valueType = valueType; }

    public String getDescription() { return description; }
    public void setDescription(String description) { this.description = description; }

    public static final class Builder {
        private final ConfigItemRequest r = new ConfigItemRequest();
        public Builder key(String v) { r.key = v; return this; }
        public Builder value(String v) { r.value = v; return this; }
        public Builder valueType(String v) { r.valueType = v; return this; }
        public Builder description(String v) { r.description = v; return this; }
        public ConfigItemRequest build() { return r; }
    }
}
