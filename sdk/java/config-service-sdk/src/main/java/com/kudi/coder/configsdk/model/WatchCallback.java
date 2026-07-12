package com.kudi.coder.configsdk.model;

/**
 * Callback interface for {@link com.kudi.coder.configsdk.ConfigServiceClient#watchNamespace}.
 *
 * <p>MVP stub: SSE streaming is intentionally not implemented. See
 * {@code ConfigServiceClient#watchNamespace} for the rationale.
 */
@FunctionalInterface
public interface WatchCallback {
    void onUpdate(ConfigItem item);
}
