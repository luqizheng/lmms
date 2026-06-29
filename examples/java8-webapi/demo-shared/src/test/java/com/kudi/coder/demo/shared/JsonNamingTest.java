package com.kudi.coder.demo.shared;

import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class JsonNamingTest {
    @Test
    void requestId_serializes_as_request_id_snake_case() throws Exception {
        ApiResponse<String> r = ApiResponse.ok("x");
        String json = new ObjectMapper().writeValueAsString(r);
        assertTrue(json.contains("\"request_id\":"), "expected snake_case field, got: " + json);
        assertFalse(json.contains("\"requestId\":"), "should not have camelCase field, got: " + json);
    }
}
