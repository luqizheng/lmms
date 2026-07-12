package com.coder.pm.llm.impl;

import com.coder.pm.llm.CompletionRequest;
import com.coder.pm.llm.CompletionResponse;
import com.coder.pm.llm.LlmProvider;
import org.springframework.stereotype.Component;

/**
 * Default {@link LlmProvider} for MVP: returns a {@code disabled} response without ever
 * calling an external service. Allows the rest of the platform to render the AI-assist
 * UI affordances without enforcing configuration.
 *
 * <p>Replaced or shadowed when a real provider ({@code OpenAiLlmProvider},
 * {@code DoubaoLlmProvider}, etc.) is registered as a {@code @Component}.
 */
@Component
public class NoopLlmProvider implements LlmProvider {

    public static final String NAME = "noop";

    @Override
    public String name() {
        return NAME;
    }

    @Override
    public boolean isEnabled() {
        return false;
    }

    @Override
    public CompletionResponse complete(CompletionRequest req) {
        return CompletionResponse.disabled(
                NAME,
                "MVP ships NoopLlmProvider only; wire a real provider to enable AI assist."
        );
    }
}
