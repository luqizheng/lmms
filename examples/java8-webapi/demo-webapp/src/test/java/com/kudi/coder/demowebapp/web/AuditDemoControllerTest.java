package com.kudi.coder.demowebapp.web;

import com.kudi.coder.auditsdk.AuditServiceClient;
import com.kudi.coder.auditsdk.model.AuditEvent;
import com.kudi.coder.auditsdk.model.AuditLog;
import com.kudi.coder.auditsdk.model.AuditLogQuery;
import com.kudi.coder.auditsdk.model.PageResult;
import com.kudi.coder.demowebapp.audit.AuditPublisher;
import com.kudi.coder.demowebapp.config.WebMvcConfig;
import com.kudi.coder.demowebapp.security.JwtAuthInterceptor;
import com.kudi.coder.demowebapp.security.PermissionAspect;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.WebMvcTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.context.annotation.Import;
import org.springframework.test.web.servlet.MockMvc;

import java.util.Collections;

import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.when;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

@WebMvcTest(AuditDemoController.class)
@Import({JwtAuthInterceptor.class, PermissionAspect.class, WebMvcConfig.class, AuditPublisher.class})
class AuditDemoControllerTest {

    @Autowired private MockMvc mvc;
    @MockBean private AuditServiceClient auditClient;

    @Test
    void sendEvent_returns_log() throws Exception {
        AuditLog log = new AuditLog();
        log.setId("log-1");
        log.setActor("alice");
        log.setAction("TASK_CREATED");
        when(auditClient.sendEvent(any(AuditEvent.class))).thenReturn(log);

        String body = "{\"actor\":\"alice\",\"action\":\"TASK_CREATED\","
                + "\"resourceType\":\"task\",\"resourceId\":\"42\"}";
        mvc.perform(post("/demo/audit/event")
                        .header("X-User-Id", "alice")
                        .contentType("application/json")
                        .content(body))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.code").value("OK"))
                .andExpect(jsonPath("$.data.id").value("log-1"));
    }

    @Test
    void queryLogs_returns_page() throws Exception {
        PageResult<AuditLog> page = new PageResult<>(Collections.<AuditLog>emptyList(), 0, 1, 20);
        when(auditClient.queryLogs(any(AuditLogQuery.class))).thenReturn(page);

        mvc.perform(get("/demo/audit/logs?actor=alice")
                        .header("X-User-Id", "alice"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.code").value("OK"))
                .andExpect(jsonPath("$.data.total").value(0));
    }
}
