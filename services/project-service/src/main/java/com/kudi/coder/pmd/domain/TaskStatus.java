package com.kudi.coder.pmd.domain;

import com.kudi.coder.pmd.web.AppException;

/**
 * Task status state machine. Transitions are governed by
 * {@link #tryTransition(TaskStatus, TaskStatus)}; illegal moves raise
 * {@link AppException} mapped to HTTP 422 with PMD_TASK_INVALID_STATE_TRANSITION.
 */
public enum TaskStatus {

    Backlog,
    Todo,
    InProgress,
    Blocked,
    Review,
    Done,
    Cancelled;

    /** Terminal states do not accept further transitions. */
    public boolean isTerminal() {
        return this == Done || this == Cancelled;
    }

    /**
     * Allowed forward transitions:
     *   Backlog → Todo · InProgress · Cancelled
     *   Todo → InProgress · Blocked · Review · Cancelled
     *   InProgress → Blocked · Review · Done · Cancelled
     *   Blocked → InProgress · Review · Cancelled
     *   Review → InProgress · Done · Cancelled
     *   Done · Cancelled — no transitions
     *
     * Same-status no-op also accepted (idempotent save).
     */
    public static void tryTransition(TaskStatus from, TaskStatus to) {
        if (from == to) return;
        if (from.isTerminal()) {
            throw AppException.invalidState(to, "from terminal " + from);
        }
        boolean ok = switch (from) {
            case Backlog -> to == Todo || to == InProgress || to == Cancelled;
            case Todo -> to == InProgress || to == Blocked || to == Review || to == Cancelled;
            case InProgress -> to == Blocked || to == Review || to == Done || to == Cancelled;
            case Blocked -> to == InProgress || to == Review || to == Cancelled;
            case Review -> to == InProgress || to == Done || to == Cancelled;
            case Done, Cancelled -> false;
        };
        if (!ok) {
            throw AppException.invalidState(to, "from " + from);
        }
    }
}
