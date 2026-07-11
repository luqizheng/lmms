package com.kudi.coder.pmd.domain;

import com.kudi.coder.pmd.web.AppException;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Pure unit test for the task state machine. Doesn't depend on Spring
 * context or a database — every case is checked against the
 * {@link TaskStatus#tryTransition} contract directly.
 */
class TaskStatusTransitionTest {

    @Test
    void sameStatusIsIdempotent() {
        assertDoesNotThrow(() -> TaskStatus.tryTransition(TaskStatus.Backlog, TaskStatus.Backlog));
        assertDoesNotThrow(() -> TaskStatus.tryTransition(TaskStatus.Done, TaskStatus.Done));
    }

    @Test
    void allowedForwardEdgesAreAccepted() {
        assertDoesNotThrow(() -> TaskStatus.tryTransition(TaskStatus.Backlog, TaskStatus.Todo));
        assertDoesNotThrow(() -> TaskStatus.tryTransition(TaskStatus.Backlog, TaskStatus.InProgress));
        assertDoesNotThrow(() -> TaskStatus.tryTransition(TaskStatus.Todo, TaskStatus.InProgress));
        assertDoesNotThrow(() -> TaskStatus.tryTransition(TaskStatus.InProgress, TaskStatus.Blocked));
        assertDoesNotThrow(() -> TaskStatus.tryTransition(TaskStatus.InProgress, TaskStatus.Review));
        assertDoesNotThrow(() -> TaskStatus.tryTransition(TaskStatus.InProgress, TaskStatus.Done));
        assertDoesNotThrow(() -> TaskStatus.tryTransition(TaskStatus.Review, TaskStatus.Done));
        assertDoesNotThrow(() -> TaskStatus.tryTransition(TaskStatus.Blocked, TaskStatus.InProgress));
    }

    @Test
    void terminalStatesRejectAll() {
        for (TaskStatus target : TaskStatus.values()) {
            AppException ex = assertThrows(AppException.class,
                    () -> TaskStatus.tryTransition(TaskStatus.Done, target));
            assertTrue(ex.code().startsWith("PMD_TASK_4001"), "code should be PMD_TASK_4001, was " + ex.code());
        }
        for (TaskStatus target : TaskStatus.values()) {
            AppException ex = assertThrows(AppException.class,
                    () -> TaskStatus.tryTransition(TaskStatus.Cancelled, target));
            assertEquals(422, ex.httpStatus());
        }
    }

    @Test
    void invalidBackwardEdgeIsRejected() {
        AppException ex = assertThrows(AppException.class,
                () -> TaskStatus.tryTransition(TaskStatus.Done, TaskStatus.InProgress));
        assertEquals(422, ex.httpStatus());
        assertEquals("PMD_TASK_4001", ex.code());
    }

    @Test
    void blockedCannotJumpToDone() {
        // Blocked → Done requires going through Review first.
        AppException ex = assertThrows(AppException.class,
                () -> TaskStatus.tryTransition(TaskStatus.Blocked, TaskStatus.Done));
        assertEquals("PMD_TASK_4001", ex.code());
    }

    @Test
    void cancelledIsTerminalLikeDone() {
        // Cancelled is terminal; only idempotent transition allowed.
        assertDoesNotThrow(() -> TaskStatus.tryTransition(TaskStatus.Cancelled, TaskStatus.Cancelled));
        assertTrue(TaskStatus.Cancelled.isTerminal());
        assertTrue(TaskStatus.Done.isTerminal());
    }
}
