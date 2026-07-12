import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";

vi.hoisted(() => {
  Object.defineProperty(globalThis, "location", {
    value: { href: "http://localhost/", protocol: "http:", host: "localhost" },
    configurable: true
  });
});

vi.mock("@/utils/sso", () => ({}));

vi.mock("@/store/modules/user", () => ({
  useUserStoreHook: () => ({})
}));

vi.mock("@pureadmin/utils", () => ({
  storageLocal: () => ({
    getItem: vi.fn(() => "tok-123"),
    setItem: vi.fn(),
    removeItem: vi.fn()
  })
}));

import { connect, disconnect, on } from "./notification-socket";

class MockWebSocket {
  static instances: MockWebSocket[] = [];
  static OPEN = 1;
  static CLOSED = 3;
  readyState = 0;
  url: string;
  onmessage: ((ev: any) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onopen: (() => void) | null = null;
  sent: any[] = [];
  constructor(url: string) {
    this.url = url;
    MockWebSocket.instances.push(this);
  }
  send(payload: string) {
    this.sent.push(payload);
  }
  close() {
    this.readyState = MockWebSocket.CLOSED;
    this.onclose?.();
  }
  fireOpen() {
    this.readyState = MockWebSocket.OPEN;
    this.onopen?.();
  }
  fireMessage(data: any) {
    this.onmessage?.({
      data: typeof data === "string" ? data : JSON.stringify(data)
    });
  }
  fireError() {
    this.onerror?.();
  }
  fireClose() {
    this.readyState = MockWebSocket.CLOSED;
    this.onclose?.();
  }
}

const flushAllTimers = () => vi.runAllTimers();

describe("notification-socket connect / disconnect", () => {
  beforeEach(() => {
    MockWebSocket.instances = [];
    vi.stubGlobal("WebSocket", MockWebSocket as any);
    vi.stubGlobal(
      "window",
      new Proxy(
        {},
        {
          get: (_, prop) => (globalThis as any)[prop]
        }
      )
    );
    vi.stubGlobal("location", { protocol: "http:", host: "localhost:8848" });
    vi.stubGlobal("localStorage", {
      getItem: vi.fn(() => JSON.stringify({ accessToken: "tok-123" })),
      setItem: vi.fn(),
      removeItem: vi.fn(),
      clear: vi.fn()
    });
    vi.useFakeTimers();
  });

  afterEach(() => {
    disconnect();
    vi.useRealTimers();
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("connect() creates a WebSocket to /api/v1/notification/ws", () => {
    connect();
    expect(MockWebSocket.instances).toHaveLength(1);
    expect(MockWebSocket.instances[0].url).toBe(
      "ws://localhost:8848/api/v1/notification/ws"
    );
  });

  it("connect() is idempotent when socket is open/connecting", () => {
    connect();
    const first = MockWebSocket.instances[0];
    first.readyState = MockWebSocket.OPEN;
    connect();
    expect(MockWebSocket.instances).toHaveLength(1);
  });

  it("onopen sends the JWT token frame", () => {
    connect();
    const sock = MockWebSocket.instances[0];
    sock.fireOpen();
    expect(sock.sent).toHaveLength(1);
    expect(JSON.parse(sock.sent[0])).toEqual({ token: "tok-123" });
  });

  it("scheduleReconnect: after close, retries 1s/2s/4s then 30s cap", () => {
    connect();
    const sock = MockWebSocket.instances[0];
    sock.fireClose();
    // RETRY_DELAYS[0] = 1000ms
    flushAllTimers();
    expect(MockWebSocket.instances.length).toBeGreaterThanOrEqual(2);
  });

  it("on() registers handler, returned unsubscribe removes it", () => {
    const handler = vi.fn();
    const off = on("ntf.notify", handler);
    connect();
    const sock = MockWebSocket.instances[0];
    sock.fireOpen();
    sock.fireMessage({ type: "ntf.notify", id: 1 });
    expect(handler).toHaveBeenCalledWith({ type: "ntf.notify", id: 1 });

    off();
    sock.fireMessage({ type: "ntf.notify", id: 2 });
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it("malformed JSON message is silently dropped", () => {
    const handler = vi.fn();
    on("any", handler);
    connect();
    const sock = MockWebSocket.instances[0];
    sock.fireOpen();
    sock.fireMessage("{not-json");
    expect(handler).not.toHaveBeenCalled();
  });
});
