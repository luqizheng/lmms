package com.coder.pm.api.llm;

import com.coder.pm.infra.llm.RequirementAnalyzer;
import com.coder.pm.infra.service.RequirementService;
import com.coder.pm.llm.CompletionResponse;
import com.coder.pm.llm.LlmProviderRegistry;
import com.coder.pm.web.exception.GlobalExceptionHandler;
import com.coder.pm.web.exception.NotFoundException;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.InjectMocks;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;
import org.springframework.test.web.servlet.MockMvc;
import org.springframework.test.web.servlet.setup.MockMvcBuilders;

import static org.mockito.ArgumentMatchers.*;
import static org.mockito.Mockito.*;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.*;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.*;

@ExtendWith(MockitoExtension.class)
class LlmControllerTest {

    private MockMvc mockMvc;

    @Mock private LlmProviderRegistry registry;
    @Mock private RequirementAnalyzer analyzer;
    @InjectMocks private LlmController controller;

    @BeforeEach
    void setup() {
        // The constructor only takes registry + analyzer, but analyzer's
        // no-arg form in turn requires RequirementService. Use a @Mock
        // for RequirementService by reflecting it into the field.
        try {
            var f = RequirementAnalyzer.class.getDeclaredField("requirementService");
            f.setAccessible(true);
            f.set(analyzer, mock(com.coder.pm.infra.service.RequirementService.class));
        } catch (Exception ignored) {}
        mockMvc = MockMvcBuilders.standaloneSetup(controller)
                .setControllerAdvice(new GlobalExceptionHandler())
                .build();
    }

    private CompletionResponse stubResponse(String provider, boolean disabled) {
        return new CompletionResponse(provider, "{}", "stop", disabled);
    }

    @Test
    void providers_returnsArray() throws Exception {
        when(registry.listProviderNames()).thenReturn(java.util.List.of("deepseek", "noop"));
        mockMvc.perform(get("/v1/llm/providers"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$[0]").value("deepseek"))
                .andExpect(jsonPath("$[1]").value("noop"));
    }

    @Test
    void decompose_usesDefaultProviderWhenParamMissing() throws Exception {
        when(analyzer.decompose(42L)).thenReturn(stubResponse("deepseek", false));
        mockMvc.perform(post("/v1/llm/requirements/42/decompose"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.provider").value("deepseek"))
                .andExpect(jsonPath("$.disabled").value(false));
        verify(analyzer).decompose(42L);
        verify(analyzer, never()).decompose(anyLong(), anyString());
    }

    @Test
    void decompose_usesExplicitProviderWhenGiven() throws Exception {
        when(analyzer.decompose(42L, "openai")).thenReturn(stubResponse("openai", false));
        mockMvc.perform(post("/v1/llm/requirements/42/decompose").param("provider", "openai"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.provider").value("openai"));
        verify(analyzer).decompose(42L, "openai");
    }

    @Test
    void decompose_returns404WhenRequirementMissing() throws Exception {
        when(analyzer.decompose(99L)).thenThrow(new NotFoundException("nf"));
        mockMvc.perform(post("/v1/llm/requirements/99/decompose"))
                .andExpect(status().isNotFound());
    }

    @Test
    void refine_usesDefaultProviderWhenParamMissing() throws Exception {
        when(analyzer.refine(7L)).thenReturn(stubResponse("deepseek", false));
        mockMvc.perform(post("/v1/llm/requirements/7/refine"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.provider").value("deepseek"));
        verify(analyzer).refine(7L);
    }

    @Test
    void refine_usesExplicitProviderWhenGiven() throws Exception {
        when(analyzer.refine(7L, "deepseek")).thenReturn(stubResponse("deepseek", true));
        mockMvc.perform(post("/v1/llm/requirements/7/refine").param("provider", "deepseek"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.disabled").value(true));
        verify(analyzer).refine(7L, "deepseek");
    }
}