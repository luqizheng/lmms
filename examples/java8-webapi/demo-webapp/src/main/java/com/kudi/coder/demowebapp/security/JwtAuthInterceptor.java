package com.kudi.coder.demowebapp.security;

import com.kudi.coder.demo.shared.ApiResponse;
import com.kudi.coder.demo.shared.ErrorCode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.MDC;
import org.springframework.stereotype.Component;
import org.springframework.web.servlet.HandlerInterceptor;

import javax.servlet.http.HttpServletRequest;
import javax.servlet.http.HttpServletResponse;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.UUID;

@Component
public class JwtAuthInterceptor implements HandlerInterceptor {

    public static final String PRINCIPAL_ATTR = "kudi.principal";
    public static final String REQUEST_ID_ATTR = "kudi.requestId";

    private static final List<String> WHITELIST = Arrays.asList(
            "/demo/auth/jwks",
            "/demo/auth/me",
            "/demo/perm/public",
            "/error"
    );

    private final ObjectMapper mapper;

    public JwtAuthInterceptor(ObjectMapper mapper) {
        this.mapper = mapper;
    }

    @Override
    public boolean preHandle(HttpServletRequest req, HttpServletResponse resp, Object handler) throws Exception {
        String path = req.getRequestURI();
        if (WHITELIST.contains(path)) {
            attachRequestId(req);
            return true;
        }

        String userId = req.getHeader("X-User-Id");
        String permsHeader = req.getHeader("X-Permissions");
        if (userId == null || userId.isEmpty()) {
            sendUnauthorized(resp, "missing X-User-Id");
            return false;
        }
        List<String> permissions = permsHeader == null || permsHeader.isEmpty()
                ? Collections.emptyList()
                : Arrays.asList(permsHeader.split(","));

        Principal p = new Principal(userId, permissions);
        req.setAttribute(PRINCIPAL_ATTR, p);
        attachRequestId(req);
        return true;
    }

    @Override
    public void afterCompletion(HttpServletRequest req, HttpServletResponse resp, Object handler, Exception ex) {
        MDC.remove("requestId");
    }

    private void attachRequestId(HttpServletRequest req) {
        String rid = req.getHeader("X-Request-Id");
        if (rid == null || rid.isEmpty()) rid = UUID.randomUUID().toString();
        req.setAttribute(REQUEST_ID_ATTR, rid);
        MDC.put("requestId", rid);
    }

    private void sendUnauthorized(HttpServletResponse resp, String msg) throws Exception {
        resp.setStatus(HttpServletResponse.SC_UNAUTHORIZED);
        resp.setContentType("application/json;charset=UTF-8");
        ApiResponse<Void> body = ApiResponse.err(ErrorCode.AUTH_TOKEN_INVALID, msg);
        resp.getWriter().write(mapper.writeValueAsString(body));
    }

    public static Principal current(HttpServletRequest req) {
        Object p = req.getAttribute(PRINCIPAL_ATTR);
        return p == null ? null : (Principal) p;
    }
}