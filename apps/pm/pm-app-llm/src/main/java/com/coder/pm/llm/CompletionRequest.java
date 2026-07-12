package com.coder.pm.llm;

/**
 * Immutable request envelope for {@link LlmProvider#complete(CompletionRequest)}.
 *
 * <p>{@code maxTokens} / {@code temperature} are advisory — implementations may ignore
 * them or map to provider-specific knobs.
 *
 * @param prompt        user-side message (required)
 * @param systemPrompt  optional system instruction; {@code null} if absent
 * @param maxTokens     optional upper bound on output tokens
 * @param temperature   optional sampling temperature (0.0–1.0+)
 */
public record CompletionRequest(
        String prompt,
        String systemPrompt,
        Integer maxTokens,
        Double temperature
) {
    public CompletionRequest {
        if (prompt == null || prompt.isBlank()) {
            throw new IllegalArgumentException("CompletionRequest.prompt must not be blank");
        }
    }

    public static Builder builder() { return new Builder(); }

    public static final class Builder {
        private String prompt;
        private String systemPrompt;
        private Integer maxTokens;
        private Double temperature;

        public Builder prompt(String v) { this.prompt = v; return this; }
        public Builder systemPrompt(String v) { this.systemPrompt = v; return this; }
        public Builder maxTokens(Integer v) { this.maxTokens = v; return this; }
        public Builder temperature(Double v) { this.temperature = v; return this; }

        public CompletionRequest build() {
            return new CompletionRequest(prompt, systemPrompt, maxTokens, temperature);
        }
    }
}
