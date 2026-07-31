package io.digua.shared;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class ApiResponseTest {

    @Test
    void ok_sets_code_OK_and_message_success() {
        ApiResponse<String> r = ApiResponse.ok("hi");
        assertEquals("OK", r.getCode());
        assertEquals("success", r.getMessage());
        assertEquals("hi", r.getData());
        assertNotNull(r.getRequestId());
    }

    @Test
    void err_keeps_null_data() {
        ApiResponse<String> r = ApiResponse.err("DEMO_X_9999", "boom");
        assertEquals("DEMO_X_9999", r.getCode());
        assertEquals("boom", r.getMessage());
        assertNull(r.getData());
    }

    @Test
    void ok_with_explicit_requestId() {
        ApiResponse<String> r = ApiResponse.ok("x", "req-1");
        assertEquals("req-1", r.getRequestId());
    }
}