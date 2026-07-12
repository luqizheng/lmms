import axios, { type AxiosInstance } from "axios";
import { getToken, formatToken } from "@/utils/auth";

const pmHttp: AxiosInstance = axios.create({
  baseURL: import.meta.env.VITE_PM_API_BASE ?? "/pm-api",
  timeout: 15000,
  headers: {
    Accept: "application/json, text/plain, */*",
    "Content-Type": "application/json",
    "X-Requested-With": "XMLHttpRequest"
  }
});

pmHttp.interceptors.request.use(config => {
  const token = getToken();
  if (token) config.headers.Authorization = formatToken(token);
  return config;
});

export interface KbSpace {
  id: number;
  projectId?: number;
  name: string;
  description?: string;
  visibility: "private" | "project" | "open";
  createdBy: number;
  createdAt: string;
  updatedAt: string;
}

export interface KbDocument {
  id: number;
  spaceId: number;
  parentId?: number;
  title: string;
  currentVersionId?: number;
  docType: "markdown" | "api_spec" | "design_doc" | "meeting";
  createdBy: number;
  updatedBy?: number;
  createdAt: string;
  updatedAt: string;
}

export interface KbDocumentVersion {
  id: number;
  documentId: number;
  versionNo: number;
  title: string;
  contentMd?: string;
  contentHtml?: string;
  editorId: number;
  editSummary?: string;
  createdAt: string;
}

export interface Paged<T> {
  items: T[];
  total: number;
  page: number;
  size: number;
}

function qs(params?: Record<string, unknown>) {
  if (!params) return "";
  const sp = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v === undefined || v === null) continue;
    sp.append(k, String(v));
  }
  const s = sp.toString();
  return s ? `?${s}` : "";
}

// ---------- spaces ----------

export function listSpaces(params: {
  page?: number;
  size?: number;
  projectId?: number;
  visibility?: string;
} = {}) {
  return pmHttp.get<Paged<KbSpace>>(`/v1/spaces${qs(params)}`);
}

export function getSpace(id: number) {
  return pmHttp.get<KbSpace>(`/v1/spaces/${id}`);
}

export function createSpace(payload: { name: string; description?: string; visibility?: string; projectId?: number }) {
  return pmHttp.post<KbSpace>("/v1/spaces", payload);
}

export function updateSpace(id: number, payload: Partial<KbSpace>) {
  return pmHttp.patch<KbSpace>(`/v1/spaces/${id}`, payload);
}

export function deleteSpace(id: number) {
  return pmHttp.delete<void>(`/v1/spaces/${id}`);
}

// ---------- documents ----------

export function listDocuments(spaceId: number, parentId?: number) {
  return pmHttp.get<KbDocument[]>(
    `/v1/spaces/${spaceId}/documents${parentId !== undefined ? `?parentId=${parentId}` : ""}`
  );
}

export function getDocument(id: number) {
  return pmHttp.get<KbDocument>(`/v1/documents/${id}`);
}

export function createDocument(spaceId: number, payload: { title: string; markdown?: string; docType?: string; parentId?: number }) {
  return pmHttp.post<KbDocument>(`/v1/spaces/${spaceId}/documents`, payload);
}

export function updateDocument(id: number, payload: Partial<KbDocument>) {
  return pmHttp.patch<KbDocument>(`/v1/documents/${id}`, payload);
}

export function updateDocumentContent(id: number, payload: { markdown: string; title?: string; editSummary?: string }) {
  return pmHttp.post<{ document: KbDocument; currentVersionId: number }>(
    `/v1/documents/${id}/content`,
    payload
  );
}

export function deleteDocument(id: number) {
  return pmHttp.delete<void>(`/v1/documents/${id}`);
}

// ---------- versions ----------

export function listVersions(documentId: number) {
  return pmHttp.get<KbDocumentVersion[]>(`/v1/documents/${documentId}/versions`);
}

export function getLatestVersion(documentId: number) {
  return pmHttp.get<KbDocumentVersion>(
    `/v1/documents/${documentId}/versions/latest`
  );
}

export function getVersion(documentId: number, n: number) {
  return pmHttp.get<KbDocumentVersion>(`/v1/documents/${documentId}/versions/${n}`);
}

export function rollbackVersion(documentId: number, n: number) {
  return pmHttp.post<KbDocumentVersion>(
    `/v1/documents/${documentId}/rollback/${n}`
  );
}

// ---------- comments (Phase D7) ----------

export interface Comment {
  id: number;
  documentId: number;
  parentId?: number;
  content: string;
  authorId: number;
  createdAt: string;
}

export function listComments(documentId: number) {
  return pmHttp.get<Comment[]>(`/v1/documents/${documentId}/comments`);
}

export function createComment(documentId: number, payload: { content: string; parentId?: number; authorId?: number }) {
  return pmHttp.post<Comment>(`/v1/documents/${documentId}/comments`, payload);
}
