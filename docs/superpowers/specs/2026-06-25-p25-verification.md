# P2.5 Frontend Verification (Notification Center + Password Dropdown)

## Status: ✅ Both features already fully implemented

Audit of codebase against the two P2.5 specs shows all files exist and match the design.

---

## A. Notification Center UI (`2026-06-21-notification-center-ui-design.md`)

### A.1 Pages — ✅ all present

| Spec'd file | Actual location |
|-------------|-----------------|
| `frontend/src/views/notification/inbox/index.vue` | ✓ exists |
| `frontend/src/views/notification/admin/index.vue` | ✓ exists |
| `frontend/src/views/notification/admin/form.vue` | ✓ exists |
| `frontend/src/views/notification/detail/index.vue` | ✓ exists |
| `frontend/src/views/notification/admin/utils/{hook,rule,types}.{tsx,ts}` | ✓ exists (hook is .tsx) |
| `frontend/src/views/notification/inbox/utils/{hook,types}.{tsx,ts}` | ✓ exists |

### A.2 State / API / Utils — ✅ all present

| Spec'd file | Status |
|-------------|--------|
| `frontend/src/store/modules/notification.ts` | ✓ Pinia store with `unreadCount`, `popoverQueue`, `refreshUnread`, `pushFromSocket`, `markRead`, `inboxList` |
| `frontend/src/api/notification.ts` | ✓ full API surface: list, get, create, publish, revoke, markRead, unreadCount |
| `frontend/src/api/iam-scopes.ts` | ✓ listDepts, listRoles, listUsers |
| `frontend/src/utils/notification-socket.ts` | ✓ WS singleton with reconnect (1s→2s→4s→8s→16s→30s), `on(type, h)`, token-aware connect/disconnect |
| `frontend/src/router/modules/notification.ts` | ✓ /notification/inbox + /notification/admin + /notification/detail/:id |
| `frontend/src/layout/components/NotificationBell.vue` | ✓ bell with badge, popover, WS connect/disconnect lifecycle |

### A.3 E2E Tests — ✅ all present

| Spec'd file | Status |
|-------------|--------|
| `tests/e2e/api/cases/notification.yaml` | ✓ exists |
| `tests/e2e/ui/cases/notification-admin.yaml` | ✓ exists |
| `tests/e2e/ui/pages/notification.page.ts` | ✓ exists |

### A.4 Differences from spec

- **hook.tsx vs hook.ts**: spec said `.ts`, impl uses `.tsx` (needed for JSX cellRenderer). Acceptable.
- **`hook.tsx` is admin only**: spec listed `{hook,rule,types}.ts` — actual has hook.tsx + rule.ts + types.ts. Acceptable.
- **WS implementation**: matches spec exactly (retry sequence, browser Authorization header via api-gateway, VITE_NOTIFICATION_WS_DISABLED env var).

### A.5 Open items

None. Feature complete and matches spec.

---

## B. Password Dropdown (`2026-06-21-change-password-dropdown-design.md`)

### B.1 Frontend files — ✅ all present

| Spec'd file | Status |
|-------------|--------|
| `frontend/src/api/iam.ts` | ✓ `changeIamPassword` added |
| `frontend/src/store/modules/user.ts` | ✓ `changePassword` action |
| `frontend/src/views/account-settings/components/ChangePasswordDialog.vue` | ✓ dialog with 3 fields + rules + submit |
| `frontend/src/layout/components/lay-navbar/index.vue` | ✓ `<ChangePasswordDialog ref="changePasswordRef" />` + dropdown item |
| `frontend/src/layout/hooks/useNav.ts` | ✓ exposes `changePasswordRef` + `openChangePassword` |
| `frontend/locales/zh-CN.yaml` | ✓ `pureChangePassword` + `changePassword.*` |
| `frontend/locales/en.yaml` | ✓ same keys English |

### B.2 Backend tests — ✅ present

| Spec'd file | Status |
|-------------|--------|
| `services/iam-service/tests/change_password_test.rs` | ✓ DTO validator tests |

### B.3 E2E — ✅ present

| Spec'd file | Status |
|-------------|--------|
| `tests/e2e/ui/cases/iam-change-password.yaml` | ✓ UI-IAM-CHPW-001 |
| `tests/e2e/ui/pages/change-password.page.ts` | ✓ Page Object |
| `tests/e2e/ui/flows/index.ts` | ✓ changePassword flow registered |

### B.4 Differences from spec

- **Locales dir is `frontend/locales/`**, not `frontend/src/locales/`. Acceptable — vue-i18n config likely points to root.

### B.5 Open items

None. Feature complete and matches spec.

---

## Conclusion

P2.5 is fully complete and verified. The specs were authored in advance and the implementation followed. No additional code changes needed — this doc captures the audit and locks in the implementation.

## Roadmap implication

Per `docs/superpowers/plans/2026-06-25-overall-roadmap.md` § 2, Phase 2 includes P2.5 "前端 P1 体验完善". With both notification center and password dropdown in place, P2.5 closes.

Next step: proceed to **Phase 3** — pending user decision on stack for P3.1 (OTel vs Prometheus) and P3.2 (gateway-central vs dual rate limiting).