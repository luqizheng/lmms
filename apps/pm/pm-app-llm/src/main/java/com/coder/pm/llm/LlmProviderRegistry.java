package com.coder.pm.llm;

import com.coder.pm.llm.impl.NoopLlmProvider;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;

import java.util.List;
import java.util.Map;
import java.util.function.Function;
import java.util.stream.Collectors;

/**
 * Discovers all {@link LlmProvider} beans at startup and dispatches completion requests
 * to the named provider.
 *
 * <p>Resolution:
 * <ol>
 *   <li>Look up provider by name (e.g. {@code "openai"}); use it if enabled.</li>
 *   <li>Fall back to {@link NoopLlmProvider} (always registered, always disabled).</li>
 *   <li>If neither found, return an empty disabled response — never throws.</li>
 * </ol>
 */
@Component
public class LlmProviderRegistry {

    private static final Logger log = LoggerFactory.getLogger(LlmProviderRegistry.class);

    private final Map<String, LlmProvider> providers;

    public LlmProviderRegistry(List<LlmProvider> beans) {
        this.providers = beans.stream().collect(Collectors.toMap(LlmProvider::name, Function.identity()));
        log.info("LLM providers registered: {}", providers.keySet());
    }

    public CompletionResponse dispatch(String name, CompletionRequest req) {
        LlmProvider target = providers.get(name);
        if (target == null) {
            log.debug("No LLM provider named '{}', falling back to noop", name);
            target = providers.get(NoopLlmProvider.NAME);
        }
        if (target == null) {
            return CompletionResponse.disabled(name, "no LLM provider bean registered");
        }
        if (!target.isEnabled()) {
            return CompletionResponse.disabled(target.name(), "provider isEnabled()=false");
        }
        try {
            return target.complete(req);
        } catch (RuntimeException ex) {
            log.warn("LLM provider '{}' threw: {}", target.name(), ex.getMessage());
            return CompletionResponse.disabled(target.name(), "provider error: " + ex.getClass().getSimpleName());
        }
    }

    public List<String> listProviderNames() {
        return List.copyOf(providers.keySet());
    }

    public boolean hasProvider(String name) {
        return providers.containsKey(name);
    }
}
