package com.coder.pm.llm;

/**
 * Response from {@link LlmProvider#complete(CompletionRequest)}.
 *
 * @param provider     name of the provider that produced this response (e.g. {@code "noop"})
 * @param content      textual content (may be empty / warning text if {@code disabled})
 * @param finishReason a short token: {@code "stop"} / {@code "length"} / {@code "disabled"} /
 *                     {@code "error"} — implementations should keep this stable for telemetry
 * @param disabled     {@code true} if the provider was not invoked (missing/disabled/error)
 */
public record CompletionResponse(
        String provider,
        String content,
        String finishReason,
        boolean disabled
) {
    public static CompletionResponse disabled(String provider, String reason) {
        return new CompletionResponse(
                provider,
                "⚠️ LLM provider '" + provider + "' is not configured. " +
                        "Reason: " + reason +
                        ". Wire a real provider in application.yml (`pm.llm.provider.default`).",
                "disabled",
                true
        );
    }
}
