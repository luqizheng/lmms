package com.kudi.coder.pmd.web;

/**
 * Domain-level exception with an error code. Mapped to HTTP by
 * {@link GlobalExceptionHandler}. The {@code code} maps directly to the
 * string identifiers in {@code contracts/error-codes.yaml} (e.g.
 * {@code PMD_PROJECT_NOT_FOUND} → HTTP 404).
 */
public class AppException extends RuntimeException {

    private final String code;
    private final int httpStatus;

    private AppException(String code, int httpStatus, String message) {
        super(message);
        this.code = code;
        this.httpStatus = httpStatus;
    }

    public String code() { return code; }
    public int httpStatus() { return httpStatus; }

    public static AppException notFound(String code, String entity, Object id) {
        return new AppException(code, 404, entity + " not found: " + id);
    }

    public static AppException conflict(String code, String message) {
        return new AppException(code, 409, message);
    }

    public static AppException invalidState(String code, String to, String fromClause) {
        return new AppException(code, 422,
                "invalid state transition to " + to + " (" + fromClause + ")");
    }

    public static AppException forbidden(String code, String message) {
        return new AppException(code, 403, message);
    }

    public static AppException badRequest(String code, String message) {
        return new AppException(code, 400, message);
    }

    public static AppException tenantMissing() {
        return new AppException("SYS_AUTH_1005", 401, "missing tenant context");
    }
}
