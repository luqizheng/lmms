package com.kudi.coder.configsdk.model;

import com.fasterxml.jackson.annotation.JsonInclude;

/**
 * Body for {@code PUT /v1/config/items/{id}}.
 */
@JsonInclude(JsonInclude.Include.NON_NULL)
public class ConfigItemUpdate {
    private String value;
    private String valueType;
    private String description;

    public ConfigItemUpdate() {}

    public static Builder builder() { return new Builder(); }

    public String getValue() { return value; }
    public void setValue(String value) { this.value = value; }

    public String getValueType() { return valueType; }
    public void setValueType(String valueType) { this.valueType = valueType; }

    public String getDescription() { return description; }
    public void setDescription(String description) { this.description = description; }

    public static final class Builder {
        private final ConfigItemUpdate r = new ConfigItemUpdate();
        public Builder value(String v) { r.value = v; return this; }
        public Builder valueType(String v) { r.valueType = v; return this; }
        public Builder description(String v) { r.description = v; return this; }
        public ConfigItemUpdate build() { return r; }
    }
}
