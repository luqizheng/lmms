package com.coder.pm.infra.service;

import com.coder.pm.domain.security.Principal;
import com.coder.pm.infra.persistence.entity.UserStory;
import com.coder.pm.infra.persistence.mapper.UserStoryMapper;
import com.coder.pm.infra.sdk.AuditEventReporter;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;
import org.springframework.security.authentication.UsernamePasswordAuthenticationToken;
import org.springframework.security.core.context.SecurityContextHolder;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.*;

@ExtendWith(MockitoExtension.class)
class UserStoryServiceTest {

    @Mock private AuditEventReporter audit;
    @Mock private UserStoryMapper mapper;

    private UserStoryService service;

    @BeforeEach
    void setup() {
        service = spy(new UserStoryService(audit));
        try {
            var field = com.baomidou.mybatisplus.extension.service.impl.ServiceImpl.class
                    .getDeclaredField("baseMapper");
            field.setAccessible(true);
            field.set(service, mapper);
        } catch (Exception ignored) {}
    }

    @AfterEach
    void clearSecurity() { SecurityContextHolder.clearContext(); }

    private void setPrincipal(Long userId, String username) {
        Principal p = new Principal(userId, username, 1L, List.of("pm_admin"), List.of("pm:*"), "access");
        SecurityContextHolder.getContext().setAuthentication(
                new UsernamePasswordAuthenticationToken(p, null, List.of()));
    }

    private UserStory validDraft() {
        UserStory s = new UserStory();
        s.setRequirementId(1L);
        s.setAsA("dev");
        s.setIWant("test");
        s.setSoThat("verified");
        return s;
    }

    @Test
    void create_throwsOnInvalidStatus() {
        UserStory s = validDraft();
        s.setStatus("not-a-status");
        assertThrows(IllegalArgumentException.class, () -> service.create(s));
        verify(service, never()).save(any());
    }

    @Test
    void create_setsDefaultStatusPendingAndSortOrderZero() {
        setPrincipal(1L, "dev");
        UserStory s = validDraft();
        doReturn(true).when(service).save(any(UserStory.class));
        UserStory result = service.create(s);
        assertEquals("pending", result.getStatus());
        assertEquals(0, result.getSortOrder());
    }

    @Test
    void create_fillsCreatedByFromPrincipal() {
        setPrincipal(88L, "carol");
        UserStory s = validDraft();
        doReturn(true).when(service).save(any(UserStory.class));
        UserStory result = service.create(s);
        assertEquals(88L, result.getCreatedBy());
        verify(audit).report(eq("carol"), eq("PM_USER_STORY_CREATED"), anyString(), anyString(), any());
    }

    @Test
    void create_usesSystemWhenNoPrincipal() {
        UserStory s = validDraft();
        doReturn(true).when(service).save(any(UserStory.class));
        service.create(s);
        verify(audit).report(eq("system"), eq("PM_USER_STORY_CREATED"), anyString(), anyString(), any());
    }
}