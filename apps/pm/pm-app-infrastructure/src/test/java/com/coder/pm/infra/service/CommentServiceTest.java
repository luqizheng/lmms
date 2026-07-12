package com.coder.pm.infra.service;

import com.baomidou.mybatisplus.core.conditions.query.QueryWrapper;
import com.coder.pm.domain.security.Principal;
import com.coder.pm.infra.persistence.entity.Comment;
import com.coder.pm.infra.persistence.mapper.CommentMapper;
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
class CommentServiceTest {

    @Mock private AuditEventReporter audit;
    @Mock private CommentMapper mapper;

    private CommentService service;

    @BeforeEach
    void setup() {
        service = spy(new CommentService(audit));
        try {
            var field = com.baomidou.mybatisplus.extension.service.impl.ServiceImpl.class
                    .getDeclaredField("baseMapper");
            field.setAccessible(true);
            field.set(service, mapper);
        } catch (Exception ignored) {}
    }

    @AfterEach
    void clearSecurity() { SecurityContextHolder.clearContext(); }

    private void setPrincipal(Long userId) {
        Principal p = new Principal(userId, "user", 1L, List.of(), List.of(), "access");
        SecurityContextHolder.getContext().setAuthentication(
                new UsernamePasswordAuthenticationToken(p, null, List.of()));
    }

    @Test
    void create_fillsAuthorIdFromPrincipal() {
        setPrincipal(42L);
        Comment c = new Comment();
        c.setDocumentId(1L);
        c.setContent("Hello");
        doAnswer(inv -> { ((Comment) inv.getArgument(0)).setId(10L); return true; })
                .when(service).save(any(Comment.class));
        Comment result = service.create(c);
        assertEquals(42L, result.getAuthorId());
        verify(audit).report(anyString(), eq("PM_COMMENT_CREATED"), anyString(), anyString(), any());
    }

    @Test
    void listByDocument_returnsOrderedComments() {
        Comment c1 = new Comment(); c1.setId(1L);
        Comment c2 = new Comment(); c2.setId(2L);
        when(service.list(any(QueryWrapper.class))).thenReturn(List.of(c1, c2));
        List<Comment> result = service.listByDocument(5L);
        assertEquals(2, result.size());
    }
}
