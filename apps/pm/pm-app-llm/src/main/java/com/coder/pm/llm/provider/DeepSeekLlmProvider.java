package com.coder.pm.llm.provider;

import com.coder.pm.llm.CompletionRequest;
import com.coder.pm.llm.CompletionResponse;
import com.coder.pm.llm.LlmProvider;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Component;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.List;
import java.util.Map;

/**
 * DeepSeek LLM provider. DeepSeek's API is OpenAI-compatible — same
 * {@code POST /chat/completions} schema — so this class can be repointed
 * at any OpenAI-compatible service (OpenAI, Azure OpenAI, local Ollama
 * with an OpenAI shim, etc.) by setting {@code pm.llm.deepseek.base-url}
 * and {@code pm.llm.deepseek.model}. Default targets DeepSeek's official
 * endpoint and {@code deepseek-chat} model.
 *
 * <p>Enable by setting:
 * <pre>
 *   pm.llm.deepseek.api-key = sk-...
 *   pm.llm.provider.default  = deepseek      (override PM_LLM_PROVIDER default)
 * </pre>
 *
 * <p>{@code isEnabled()} returns false until {@code api-key} is non-blank,
 * so this bean is harmless to register when DeepSeek is not configured
 * (the registry falls back to {@code noop} and returns a disabled
 * response — same behaviour as before this class existed).
 */
@Component(DeepSeekLlmProvider.NAME)
public class DeepSeekLlmProvider implements LlmProvider {

    public static final String NAME = "deepseek";

    private static final Logger log = LoggerFactory.getLogger(DeepSeekLlmProvider.class);
    private static final String CHAT_COMPLETIONS_PATH = "/chat/completions";
    private static final Duration HTTP_TIMEOUT = Duration.ofSeconds(60);
    private static final int CONNECT_TIMEOUT_SEC = 10;

    private final String apiKey;
    private final String baseUrl;
    private final String model;
    private final HttpClient http;
    private final ObjectMapper mapper;

    public DeepSeekLlmProvider(
            @Value("${pm.llm.deepseek.api-key:}") String apiKey,
            @Value("${pm.llm.deepseek.base-url:https://api.deepseek.com}") String baseUrl,
            @Value("${pm.llm.deepseek.model:deepseek-v4-flash}") String model,
            ObjectMapper mapper) {
        this.apiKey = apiKey;
        this.baseUrl = baseUrl;
        this.model = model;
        this.http = HttpClient.newBuilder()
                .connectTimeout(Duration.ofSeconds(CONNECT_TIMEOUT_SEC))
                .build();
        this.mapper = mapper;
    }

    @Override
    public String name() { return NAME; }

    @Override
    public boolean isEnabled() {
        return apiKey != null && !apiKey.isBlank();
    }

    @Override
    public CompletionResponse complete(CompletionRequest req) {
        if (!isEnabled()) {
            return CompletionResponse.disabled(name(), "pm.llm.deepseek.api-key not configured");
        }
        try {
            String body = mapper.writeValueAsString(Map.of(
                    "model", model,
                    "messages", List.of(
                            Map.of("role", "system", "content",
                                    req.systemPrompt() == null ? "" : req.systemPrompt()),
                            Map.of("role", "user", "content", req.prompt())),
                    "max_tokens", req.maxTokens() == null ? 1024 : req.maxTokens(),
                    "temperature", req.temperature() == null ? 0.3 : req.temperature(),
                    "stream", false));

            HttpRequest httpReq = HttpRequest.newBuilder()
                    .uri(URI.create(baseUrl + CHAT_COMPLETIONS_PATH))
                    .timeout(HTTP_TIMEOUT)
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer " + apiKey)
                    .POST(HttpRequest.BodyPublishers.ofString(body))
                    .build();

            HttpResponse<String> resp = http.send(httpReq, HttpResponse.BodyHandlers.ofString());
            int status = resp.statusCode();
            String respBody = resp.body();
            if (status / 100 != 2) {
                log.warn("DeepSeek API non-2xx status={} body={}", status, truncate(respBody, 200));
                return new CompletionResponse(name(),
                        "DeepSeek API error HTTP " + status + ": " + truncate(respBody, 200),
                        "error", true);
            }

            @SuppressWarnings("unchecked")
            Map<String, Object> respMap = mapper.readValue(respBody, Map.class);
            Object choicesObj = respMap.get("choices");
            if (!(choicesObj instanceof List<?> choices) || choices.isEmpty()) {
                return CompletionResponse.disabled(name(), "empty choices");
            }
            Object first = choices.get(0);
            if (!(first instanceof Map<?, ?> firstChoice)) {
                return CompletionResponse.disabled(name(), "malformed choice");
            }
            Object messageObj = firstChoice.get("message");
            if (!(messageObj instanceof Map<?, ?> message)) {
                return CompletionResponse.disabled(name(), "no message in choice");
            }
            Object contentObj = message.get("content");
            String content = contentObj == null ? "" : contentObj.toString();
            return new CompletionResponse(name(), content, "stop", false);
        } catch (Exception ex) {
            log.warn("DeepSeek call failed: {}", ex.getMessage());
            return new CompletionResponse(name(),
                    "DeepSeek call failed: " + ex.getClass().getSimpleName() + ": " + ex.getMessage(),
                    "error", true);
        }
    }

    private static String truncate(String s, int max) {
        if (s == null) return "";
        return s.length() <= max ? s : s.substring(0, max) + "…";
    }
}
