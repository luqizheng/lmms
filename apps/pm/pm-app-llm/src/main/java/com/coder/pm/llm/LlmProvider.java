package com.coder.pm.llm;

/**
 * Abstraction over an LLM provider.
 *
 * <p>MVP ships {@link com.coder.pm.llm.impl.NoopLlmProvider} which always returns a
 * "disabled" response. To wire a real provider (OpenAI / Doubao / Ollama), implement this
 * interface, annotate with {@code @Component}, and rely on
 * {@link LlmProviderRegistry#dispatch(String, CompletionRequest)} for selection.
 *
 * <p><strong>Design note:</strong> we keep this in {@code pm-app-llm} as a separate module
 * so other future consumers (e.g. a comment summarization service) can depend on the LLM
 * abstraction without pulling in domain code.
 */
public interface LlmProvider {

    /**
     * Unique provider name used by {@link LlmProviderRegistry#dispatch(String, CompletionRequest)}.
     * Examples: {@code "noop"}, {@code "openai"}, {@code "doubao"}, {@code "ollama"}.
     */
    String name();

    /**
     * Indicates whether this provider is enabled.
     *
     * <p>If {@code false}, the registry short-circuits with a {@code disabled=true} response
     * and does not invoke {@link #complete(CompletionRequest)}. Useful for ops toggling
     * providers on/off via config.
     */
    boolean isEnabled();

    /**
     * Run a completion synchronously.
     *
     * <p>Implementations should treat {@link CompletionRequest#prompt()} as the user message
     * and {@link CompletionRequest#systemPrompt()} as the (optional) system instruction.
     * Implementation must never throw on benign input — return a {@code disabled}-like
     * response on quota / network errors so the caller can degrade gracefully.
     */
    CompletionResponse complete(CompletionRequest req);
}
