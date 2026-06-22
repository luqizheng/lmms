import { storageLocal } from "@pureadmin/utils";
import { TokenKey } from "@/utils/auth";

type Handler = (msg: any) => void;
type WsMessage = {
  type: string;
  id?: number;
  title?: string;
  count?: number;
};

let socket: WebSocket | null = null;
let retry = 0;
let retryTimer: number | null = null;
const handlers = new Map<string, Set<Handler>>();

const RETRY_DELAYS = [1000, 2000, 4000, 8000, 16000, 30000];

function getToken(): string | null {
  return storageLocal().getItem<string>(TokenKey);
}

function wsUrl() {
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  return `${proto}//${location.host}/api/v1/notification/ws`;
}

function scheduleReconnect() {
  if (retryTimer != null) return;
  const delay = RETRY_DELAYS[Math.min(retry, RETRY_DELAYS.length - 1)];
  retry += 1;
  retryTimer = window.setTimeout(() => {
    retryTimer = null;
    open();
  }, delay);
}

function open() {
  try {
    socket = new WebSocket(wsUrl());
  } catch {
    scheduleReconnect();
    return;
  }
  socket.onmessage = ev => {
    let msg: WsMessage;
    try {
      msg = JSON.parse(ev.data);
    } catch {
      return;
    }
    handlers.get(msg.type)?.forEach(h => h(msg));
    handlers.get("*")?.forEach(h => h(msg));
  };
  socket.onclose = () => {
    socket = null;
    if (retry < 5) scheduleReconnect();
  };
  socket.onerror = () => {
    socket?.close();
  };
  socket.onopen = () => {
    retry = 0;
    const token = getToken();
    if (token) {
      socket?.send(JSON.stringify({ token }));
    }
  };
}

export function connect() {
  if (socket && socket.readyState <= 1) return;
  if (import.meta.env.VITE_NOTIFICATION_WS_DISABLED === "true") return;
  open();
}

export function disconnect() {
  if (retryTimer != null) {
    clearTimeout(retryTimer);
    retryTimer = null;
  }
  if (socket) {
    socket.onclose = null;
    socket.close();
    socket = null;
  }
  retry = 0;
}

export function on(type: string, h: Handler) {
  if (!handlers.has(type)) handlers.set(type, new Set());
  handlers.get(type)!.add(h);
  return () => {
    handlers.get(type)?.delete(h);
  };
}
