package com.kudi.coder.pmd.config;

import jakarta.servlet.FilterChain;
import jakarta.servlet.ServletException;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;
import org.springframework.web.filter.OncePerRequestFilter;

import java.io.IOException;
import java.util.Base64;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;

/**
 * 解码 {@code Authorization: Bearer <jwt>} payload（信任网关已验签），从中提取
 * <code>tid</code>（tenant id）与 <code>sub</code>（user id）写入 {@link TenantContext}。
 *
 * 网关已 RS256 验签 + 黑名单检查 + exp 检查，本服务仅按约定从 payload 抽取上下文，**不**重新验签。
 * 内部 <code>/project/internal/*</code> 端点要求 service-jwt（aud claim 校验由调用方负责）；
 * 该 filter 对内部端点同样解析，但本地调用通常已经注入 tenant header。
 */
@Component
public class TenantContextFilter extends OncePerRequestFilter {

    private static final Logger log = LoggerFactory.getLogger(TenantContextFilter.class);
    private static final ObjectMapper MAPPER = new ObjectMapper();

    @Override
    protected void doFilterInternal(HttpServletRequest request,
                                    HttpServletResponse response,
                                    FilterChain chain) throws ServletException, IOException {
        try {
            String header = request.getHeader("Authorization");
            if (header != null && header.startsWith("Bearer ")) {
                String token = header.substring("Bearer ".length()).trim();
                JsonNode payload = decodePayload(token);
                if (payload != null) {
                    String tid = payload.path("tid").asText(null);
                    String sub = payload.path("sub").asText(null);
                    if (tid != null) {
                        TenantContext.setTenantId(Long.parseLong(tid));
                    }
                    if (sub != null) {
                        TenantContext.setUserId(sub);
                    }
                }
            }
        } catch (Exception e) {
            log.warn("tenant context decode failed, request proceeds without context: {}", e.getMessage());
        }
        try {
            chain.doFilter(request, response);
        } finally {
            TenantContext.clear();
        }
    }

    private static JsonNode decodePayload(String jwt) {
        int firstDot = jwt.indexOf('.');
        if (firstDot < 0) return null;
        int secondDot = jwt.indexOf('.', firstDot + 1);
        if (secondDot < 0) return null;
        String body = jwt.substring(firstDot + 1, secondDot);
        try {
            byte[] decoded = Base64.getUrlDecoder().decode(body);
            return MAPPER.readTree(decoded);
        } catch (IllegalArgumentException e) {
            return null;
        }
    }
}
