// @vitest-environment happy-dom
import { describe, expect, it, vi, beforeEach } from "vitest";
import { nextTick } from "vue";

const { useNotificationStore, connect, disconnect, on } = vi.hoisted(() => ({
  useNotificationStore: vi.fn(),
  connect: vi.fn(),
  disconnect: vi.fn(),
  on: vi.fn().mockReturnValue(() => {})
}));

vi.mock("@/store/modules/notification", () => ({
  useNotificationStore,
  default: useNotificationStore
}));
vi.mock("@/utils/notification-socket", () => ({ connect, disconnect, on }));
vi.mock("vue-i18n", () => ({
  useI18n: () => ({
    t: (k: string, params?: Record<string, unknown>) =>
      params ? `${k}|${JSON.stringify(params)}` : k
  }),
  createI18n: () => ({}),
  default: {}
}));
vi.mock("vue-router", () => ({
  useRouter: () => ({ push: vi.fn() })
}));

import NotificationBell from "./NotificationBell.vue";

const mockStore = {
  refreshUnread: vi.fn().mockResolvedValue(undefined),
  refreshInbox: vi.fn().mockResolvedValue(undefined),
  pushFromSocket: vi.fn(),
  unreadCount: 0,
  popoverQueue: [],
  inboxList: []
};

let mockPermission: NotificationPermission = "default";
let announceHandler: ((msg: any) => void) | undefined;
const mockNotification = vi.fn();
const mockReqPermission = vi.fn();

function setupMockNotification() {
  // mockReqPermission 始终 mockResolvedValue("granted")
  mockReqPermission.mockResolvedValue("granted");
  mockReqPermission.mockClear();

  class MockNotificationClass {
    static permission: NotificationPermission = mockPermission;
    static requestPermission = mockReqPermission;
    title: string;
    options?: NotificationOptions;
    onclick: (() => void) | null = null;
    close = vi.fn();
    constructor(title: string, options?: NotificationOptions) {
      this.title = title;
      this.options = options;
      mockNotification(title, options);
    }
  }

  // 必须 delete 再 define，否则 Object.defineProperty 会抛错
  delete (window as any).Notification;
  Object.defineProperty(window, "Notification", {
    value: MockNotificationClass,
    configurable: true,
    writable: true
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  useNotificationStore.mockReturnValue(mockStore);
  mockNotification.mockClear();
  announceHandler = undefined;
  (on as any).mockImplementation((evt: string, cb: any) => {
    if (evt === "announcement") announceHandler = cb;
    return () => {};
  });
  setupMockNotification();
});

async function mountBell() {
  const { mount } = await import("@vue/test-utils");
  const wrapper = mount(NotificationBell);
  await nextTick();
  return wrapper;
}

describe("NotificationBell desktop notification + audio cue", () => {
  it("requests browser notification permission on mount (default → ask)", async () => {
    mockPermission = "default";
    setupMockNotification();
    await mountBell();
    expect(mockReqPermission).toHaveBeenCalled();
  });

  it("does not request permission when already granted", async () => {
    mockPermission = "granted";
    setupMockNotification();
    await mountBell();
    expect(mockReqPermission).not.toHaveBeenCalled();
  });

  it("fires desktop notification on announcement push when permission granted", async () => {
    mockPermission = "granted";
    setupMockNotification();
    await mountBell();
    announceHandler?.({ id: 1, title: "测试公告" });
    expect(mockStore.pushFromSocket).toHaveBeenCalledWith({
      id: 1,
      title: "测试公告"
    });
    expect(mockNotification).toHaveBeenCalledTimes(1);
    expect(mockNotification).toHaveBeenCalledWith(
      "menus.notificationDesktopTitle",
      expect.objectContaining({
        body: expect.stringContaining("测试公告")
      })
    );
  });

  it("does not fire desktop notification when permission denied", async () => {
    mockPermission = "denied";
    setupMockNotification();
    await mountBell();
    announceHandler?.({ id: 2, title: "其他" });
    expect(mockNotification).not.toHaveBeenCalled();
  });
});
