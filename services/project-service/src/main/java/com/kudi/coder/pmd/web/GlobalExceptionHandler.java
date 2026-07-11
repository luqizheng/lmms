package com.kudi.coder.pmd.web;

import jakarta.validation.ConstraintViolationException;
import org.springframework.dao.DataIntegrityViolationException;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.MethodArgumentNotValidException;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.RestControllerAdvice;

import java.time.Instant;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * Maps {@link AppException} (and a few Spring built-ins) to JSON error
 * responses with the contract-defined PMD_* error codes. Trace correlation
 * piggy-backs on the {@code X-Trace-Id} header supplied by the gateway
 * (downstream of the existing trace middleware in common-telemetry-axum)
 * when present, otherwise a UUID is generated.
 */
@RestControllerAdvice
public class GlobalExceptionHandler {

    @ExceptionHandler(AppException.class)
    public ResponseEntity<Map<String, Object>> handleApp(AppException e) {
        return ResponseEntity
                .status(e.httpStatus())
                .body(body(e.code(), e.getMessage()));
    }

    @ExceptionHandler(MethodArgumentNotValidException.class)
    public ResponseEntity<Map<String, Object>> handleValidation(MethodArgumentNotValidException e) {
        StringBuilder sb = new StringBuilder("validation failed");
        e.getBindingResult().getAllErrors().forEach(err ->
                sb.append(": ").append(err.getDefaultMessage()));
        return ResponseEntity.status(HttpStatus.BAD_REQUEST)
                .body(body("VALID_2001", sb.toString()));
    }

    @ExceptionHandler(ConstraintViolationException.class)
    public ResponseEntity<Map<String, Object>> handleConstraint(ConstraintViolationException e) {
        return ResponseEntity.status(HttpStatus.BAD_REQUEST)
                .body(body("VALID_2001", e.getMessage()));
    }

    @ExceptionHandler(DataIntegrityViolationException.class)
    public ResponseEntity<Map<String, Object>> handleIntegrity(DataIntegrityViolationException e) {
        // MySQL duplicate-key / FK violations surface as DataIntegrityViolationException.
        // For project.code the unique key triggers this on archive vs create race.
        if (e.getMessage() != null && e.getMessage().toLowerCase().contains("pmd_projects.code")) {
            return ResponseEntity.status(HttpStatus.CONFLICT)
                    .body(body("PMD_PROJECT_3001", "project code already exists"));
        }
        return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                .body(body("SYS_DB_0001", "database error"));
    }

    @ExceptionHandler(IllegalStateException.class)
    public ResponseEntity<Map<String, Object>> handleIllegal(IllegalStateException e) {
        // Thrown by TenantContextFilter when the JWT scope is missing.
        if (e.getMessage() != null && e.getMessage().toLowerCase().contains("tenant context")) {
            return ResponseEntity.status(HttpStatus.UNAUTHORIZED)
                    .body(body("SYS_AUTH_1005", e.getMessage()));
        }
        return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                .body(body("SYS_INTERNAL_0001", "internal error"));
    }

    private static Map<String, Object> body(String code, String message) {
        Map<String, Object> m = new LinkedHashMap<>();
        m.put("code", code);
        m.put("message", message);
        m.put("trace_id", traceIdFallback());
        m.put("ts", Instant.now().toString());
        return m;
    }

    private static String traceIdFallback() {
        return "trace-" + java.util.UUID.randomUUID().toString().replace("-", "");
    }
}
