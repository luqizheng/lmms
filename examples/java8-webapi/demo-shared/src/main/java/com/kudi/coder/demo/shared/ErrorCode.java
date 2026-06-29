package com.kudi.coder.demo.shared;

public final class ErrorCode {

    public static final String OK = "OK";

    public static final String AUTH_INVALID_CRED  = "DEMO_AUTH_1001";
    public static final String AUTH_TOKEN_INVALID = "DEMO_AUTH_1002";

    public static final String PERM_DENIED = "DEMO_PERM_2001";

    public static final String TASK_NOT_FOUND    = "DEMO_TASK_3001";
    public static final String TASK_ILLEGAL_STATE = "DEMO_TASK_3002";

    public static final String VALIDATION_FAILED = "DEMO_VALID_4001";

    public static final String UPSTREAM_FAILURE  = "DEMO_UPSTREAM_5001";

    private ErrorCode() {}
}
