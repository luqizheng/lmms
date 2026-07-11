package com.kudi.coder.pmd.llm;

import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

/**
 * Provider factory. Only {@link PlaceholderLlmProvider} exists today (selected
 * by env {@code pmd.llm.provider=placeholder}); HTTP-backed providers (Ark /
 * Ollama / local) land in a follow-up Phase 10 spec and plug in here behind
 * the same {@link LlmProvider} interface.
 *
 * Spring autowires {@link PlaceholderLlmProvider} as the default because of
 * its {@code @ConditionalOnProperty(matchIfMissing = true)}; this config
 * class only exists so future providers have a single seam to register.
 */
@Configuration
public class LlmConfig {

    @Bean
    public LlmProvider provider(PlaceholderLlmProvider placeholder) {
        return placeholder;
    }
}
