package com.coder.pm.web.exception;

import jakarta.servlet.http.HttpServletRequest;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.validation.FieldError;
import org.springframework.web.bind.MethodArgumentNotValidException;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.RestControllerAdvice;

import java.time.Instant;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * Unified error envelope.
 *
 * <pre>{@code
 * { "code": 40001, "message": "validation failed", "traceId": "...", "data": null }
 * }</pre>
 *
 * <p>Codes: {@code 1xxxx} business, {@code 9xxxx} infra/system, {@code 40001} validation,
 * {@code 40401} not found, {@code 40901} conflict, {@code 50001} internal.
 */
@RestControllerAdvice
public class GlobalExceptionHandler {

    private static final Logger log = LoggerFactory.getLogger(GlobalExceptionHandler.class);

    @ExceptionHandler(MethodArgumentNotValidException.class)
    public ResponseEntity<Map<String, Object>> onValidation(MethodArgumentNotValidException ex,
                                                            HttpServletRequest req) {
        List<String> details = ex.getBindingResult().getFieldErrors().stream()
                .map(this::formatFieldError).toList();
        return ResponseEntity.status(HttpStatus.BAD_REQUEST).body(envelope(
                40001, "validation failed", req, Map.of("errors", details)));
    }

    @ExceptionHandler(IllegalArgumentException.class)
    public ResponseEntity<Map<String, Object>> onIllegalArg(IllegalArgumentException ex,
                                                            HttpServletRequest req) {
        return ResponseEntity.status(HttpStatus.BAD_REQUEST).body(envelope(
                40000, ex.getMessage() == null ? "bad request" : ex.getMessage(), req, null));
    }

    @ExceptionHandler(NotFoundException.class)
    public ResponseEntity<Map<String, Object>> onNotFound(NotFoundException ex, HttpServletRequest req) {
        return ResponseEntity.status(HttpStatus.NOT_FOUND).body(envelope(40401, ex.getMessage(), req, null));
    }

    @ExceptionHandler(ConflictException.class)
    public ResponseEntity<Map<String, Object>> onConflict(ConflictException ex, HttpServletRequest req) {
        return ResponseEntity.status(HttpStatus.CONFLICT).body(envelope(40901, ex.getMessage(), req, null));
    }

    @ExceptionHandler(Exception.class)
    public ResponseEntity<Map<String, Object>> onAny(Exception ex, HttpServletRequest req) {
        String traceId = UUID.randomUUID().toString();
        log.error("unhandled exception traceId={} path={}", traceId, req.getRequestURI(), ex);
        return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR).body(envelope(
                50001, "internal server error", req, Map.of("traceId", traceId)));
    }

    private Map<String, Object> envelope(int code, String message, HttpServletRequest req, Object data) {
        Map<String, Object> body = new LinkedHashMap<>();
        body.put("code", code);
        body.put("message", message);
        body.put("path", req.getRequestURI());
        body.put("timestamp", Instant.now().toString());
        body.put("data", data);
        return body;
    }

    private String formatFieldError(FieldError fe) {
        return fe.getField() + ": " + fe.getDefaultMessage();
    }
}
