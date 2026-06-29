package com.kudi.coder.demowebapp.security;

import com.kudi.coder.demo.shared.ApiResponse;
import com.kudi.coder.demo.shared.ErrorCode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.aspectj.lang.ProceedingJoinPoint;
import org.aspectj.lang.annotation.Around;
import org.aspectj.lang.annotation.Aspect;
import org.aspectj.lang.reflect.MethodSignature;
import org.springframework.stereotype.Component;
import org.springframework.web.context.request.RequestContextHolder;
import org.springframework.web.context.request.ServletRequestAttributes;

import javax.servlet.http.HttpServletRequest;
import javax.servlet.http.HttpServletResponse;
import java.lang.reflect.Method;

@Aspect
@Component
public class PermissionAspect {

    private final ObjectMapper mapper;

    public PermissionAspect(ObjectMapper mapper) {
        this.mapper = mapper;
    }

    @Around("@annotation(com.kudi.coder.demowebapp.security.RequiresPermission)")
    public Object check(ProceedingJoinPoint pjp) throws Throwable {
        MethodSignature sig = (MethodSignature) pjp.getSignature();
        Method method = sig.getMethod();
        RequiresPermission rp = method.getAnnotation(RequiresPermission.class);
        if (rp == null) return pjp.proceed();

        ServletRequestAttributes attrs = (ServletRequestAttributes) RequestContextHolder.getRequestAttributes();
        if (attrs == null) return pjp.proceed();
        HttpServletRequest req = attrs.getRequest();
        Principal principal = JwtAuthInterceptor.current(req);
        if (principal == null || !principal.hasPermission(rp.value())) {
            HttpServletResponse resp = attrs.getResponse();
            resp.setStatus(HttpServletResponse.SC_FORBIDDEN);
            resp.setContentType("application/json;charset=UTF-8");
            ApiResponse<Void> body = ApiResponse.err(ErrorCode.PERM_DENIED,
                    "missing permission: " + rp.value());
            resp.getWriter().write(mapper.writeValueAsString(body));
            return null;
        }
        return pjp.proceed();
    }
}