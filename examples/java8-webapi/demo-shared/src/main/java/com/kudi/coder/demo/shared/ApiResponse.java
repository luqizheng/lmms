package com.kudi.coder.demo.shared;

import com.fasterxml.jackson.annotation.JsonInclude;

import java.util.UUID;

@JsonInclude(JsonInclude.Include.NON_NULL)
public class ApiResponse<T> {

    public static final String OK = "OK";

    private String code;
    private String message;
    private T data;
    private String requestId;

    public ApiResponse() {}

    public ApiResponse(String code, String message, T data, String requestId) {
        this.code = code;
        this.message = message;
        this.data = data;
        this.requestId = requestId;
    }

    public static <T> ApiResponse<T> ok(T data) {
        return new ApiResponse<>(OK, "success", data, newRequestId());
    }

    public static <T> ApiResponse<T> ok(T data, String requestId) {
        return new ApiResponse<>(OK, "success", data, requestId);
    }

    public static <T> ApiResponse<T> err(String code, String message) {
        return new ApiResponse<>(code, message, null, newRequestId());
    }

    public static <T> ApiResponse<T> err(String code, String message, String requestId) {
        return new ApiResponse<>(code, message, null, requestId);
    }

    private static String newRequestId() {
        return UUID.randomUUID().toString();
    }

    public String getCode() { return code; }
    public void setCode(String code) { this.code = code; }
    public String getMessage() { return message; }
    public void setMessage(String message) { this.message = message; }
    public T getData() { return data; }
    public void setData(T data) { this.data = data; }
    public String getRequestId() { return requestId; }
    public void setRequestId(String requestId) { this.requestId = requestId; }
}
