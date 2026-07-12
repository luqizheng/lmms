package com.coder.pm.infra.service;

import com.coder.pm.domain.security.Principal;
import com.coder.pm.infra.persistence.entity.AcceptanceCriteria;
import com.coder.pm.infra.persistence.mapper.AcceptanceCriteriaMapper;
import com.coder.pm.infra.sdk.AuditEventReporter;
import com.coder.pm.web.exception.NotFoundException;
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
class AcceptanceCriteriaServiceTest {

    @Mock private AuditEventReporter audit;
    @Mock private AcceptanceCriteriaMapper mapper;

    private AcceptanceCriteriaService service;

    @BeforeEach
    void setup() {
        service = spy(new AcceptanceCriteriaService(audit));
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

    private AcceptanceCriteria validDraft() {
        AcceptanceCriteria ac = new AcceptanceCriteria();
        ac.setUserStoryId(1L);
        ac.setGivenText("given");
        ac.setWhenText("when");
        ac.setThenText("then");
        return ac;
    }

    @Test
    void create_setsDefaultsCheckedFalseAndSortOrderZero() {
        setPrincipal(1L, "dev");
        AcceptanceCriteria ac = validDraft();
        doReturn(true).when(service).save(any(AcceptanceCriteria.class));
        AcceptanceCriteria result = service.create(ac);
        assertFalse(result.getChecked());
        assertEquals(0, result.getSortOrder());
    }

    @Test
    void create_firesAuditWithActorName() {
        setPrincipal(33L, "eve");
        AcceptanceCriteria ac = validDraft();
        doReturn(true).when(service).save(any(AcceptanceCriteria.class));
        service.create(ac);
        verify(audit).report(eq("eve"), eq("PM_AC_CREATED"), anyString(), anyString(), any());
    }

    @Test
    void check_setsCheckedTrueAndFiresCheckedEvent() {
        setPrincipal(1L, "dev");
        AcceptanceCriteria existing = validDraft();
        existing.setId(10L);
        existing.setChecked(false);

        doReturn(existing).when(service).getById(10L);
        doReturn(true).when(service).updateById(any(AcceptanceCriteria.class));

        AcceptanceCriteria result = service.check(10L, true);

        assertTrue(result.getChecked());
        verify(audit).report(eq("dev"), eq("PM_AC_CHECKED"), anyString(), anyString(), any());
    }

    @Test
    void check_setsCheckedFalseAndFiresUncheckedEvent() {
        setPrincipal(1L, "dev");
        AcceptanceCriteria existing = validDraft();
        existing.setId(11L);
        existing.setChecked(true);

        doReturn(existing).when(service).getById(11L);
        doReturn(true).when(service).updateById(any(AcceptanceCriteria.class));

        AcceptanceCriteria result = service.check(11L, false);

        assertFalse(result.getChecked());
        verify(audit).report(eq("dev"), eq("PM_AC_UNCHECKED"), anyString(), anyString(), any());
    }

    @Test
    void check_throwsNotFoundWhenMissing() {
        doReturn(null).when(service).getById(999L);
        assertThrows(NotFoundException.class, () -> service.check(999L, true));
    }

    @Test
    void update_throwsNotFoundWhenMissing() {
        doReturn(null).when(service).getById(888L);
        assertThrows(NotFoundException.class, () -> service.update(888L, new AcceptanceCriteria()));
    }

    @Test
    void update_appliesPatchesAndFiresAudit() {
        setPrincipal(1L, "dev");
        AcceptanceCriteria existing = validDraft();
        existing.setId(5L);
        existing.setChecked(false);

        AcceptanceCriteria patch = new AcceptanceCriteria();
        patch.setChecked(true);
        patch.setGivenText("updated given");

        doReturn(existing).when(service).getById(5L);
        doReturn(true).when(service).updateById(any(AcceptanceCriteria.class));

        AcceptanceCriteria result = service.update(5L, patch);

        assertTrue(result.getChecked());
        assertEquals("updated given", result.getGivenText());
        verify(audit).report(eq("dev"), eq("PM_AC_UPDATED"), anyString(), anyString(), any());
    }
}