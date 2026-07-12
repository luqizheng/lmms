import axios, { type AxiosInstance } from "axios";
import { getToken, formatToken } from "@/utils/auth";

// PM Platform services talk to Spring Boot at `/pm-api` directly (not via api-gateway).
// Backend spec: docs/superpowers/specs/2026-07-11-pm-platform-design.md §4.1.

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

export interface Project {
  id: number;
  code: string;
  name: string;
  description?: string;
  status: "active" | "archived";
  ownerId: number;
  createdBy: number;
  createdAt: string;
  updatedAt: string;
}

export interface Paged<T> {
  items: T[];
  total: number;
  page: number;
  size: number;
}

export interface Requirement {
  id: number;
  projectId: number;
  parentId?: number;
  title: string;
  description?: string;
  type: "feature" | "improvement" | "bug" | "task";
  priority: "p0" | "p1" | "p2" | "p3";
  status: "draft" | "open" | "in_progress" | "resolved" | "closed" | "rejected";
  estimateHours?: number;
  assigneeId?: number;
  createdBy: number;
  createdAt: string;
  updatedAt: string;
}

export interface UserStory {
  id: number;
  requirementId: number;
  asA?: string;
  iWant?: string;
  soThat?: string;
  storyPoints?: number;
  status: "pending" | "in_progress" | "done" | "rejected";
  sortOrder: number;
  createdBy: number;
  createdAt: string;
  updatedAt: string;
}

export interface AcceptanceCriteria {
  id: number;
  userStoryId: number;
  givenText?: string;
  whenText?: string;
  thenText?: string;
  checked: boolean;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
}

export interface Task {
  id: number;
  projectId: number;
  requirementId?: number;
  title: string;
  description?: string;
  status: "todo" | "in_progress" | "done";
  priority: "low" | "medium" | "high";
  assigneeId?: number;
  dueDate?: string;
  sortOrder: number;
  createdBy: number;
  createdAt: string;
  updatedAt: string;
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

// ---------- projects ----------

export function listProjects(params: { page?: number; size?: number; status?: string } = {}) {
  return pmHttp.get<Paged<Project>>(`/v1/projects${qs(params)}`);
}

export function getProject(id: number) {
  return pmHttp.get<Project>(`/v1/projects/${id}`);
}

export function createProject(payload: Pick<Project, "code" | "name"> & Partial<Project>) {
  return pmHttp.post<Project>("/v1/projects", payload);
}

export function updateProject(id: number, payload: Partial<Project>) {
  return pmHttp.patch<Project>(`/v1/projects/${id}`, payload);
}

export function archiveProject(id: number) {
  return pmHttp.delete<void>(`/v1/projects/${id}`);
}

// ---------- requirements ----------

export function listRequirements(
  projectId: number,
  params: { page?: number; size?: number; status?: string; type?: string } = {}
) {
  return pmHttp.get<Paged<Requirement>>(
    `/v1/projects/${projectId}/requirements${qs(params)}`
  );
}

export function getRequirement(id: number) {
  return pmHttp.get<Requirement>(`/v1/requirements/${id}`);
}

export function createRequirement(projectId: number, payload: Partial<Requirement> & { title: string; type: string }) {
  return pmHttp.post<Requirement>(`/v1/projects/${projectId}/requirements`, payload);
}

export function updateRequirement(id: number, payload: Partial<Requirement>) {
  return pmHttp.patch<Requirement>(`/v1/requirements/${id}`, payload);
}

export function deleteRequirement(id: number) {
  return pmHttp.delete<void>(`/v1/requirements/${id}`);
}

// ---------- user stories ----------

export function listStories(requirementId: number) {
  return pmHttp.get<UserStory[]>(`/v1/requirements/${requirementId}/stories`);
}

export function createStory(requirementId: number, payload: Partial<UserStory>) {
  return pmHttp.post<UserStory>(`/v1/requirements/${requirementId}/stories`, payload);
}

export function updateStory(id: number, payload: Partial<UserStory>) {
  return pmHttp.patch<UserStory>(`/v1/stories/${id}`, payload);
}

// ---------- acceptance criteria ----------

export function listCriteria(storyId: number) {
  return pmHttp.get<AcceptanceCriteria[]>(`/v1/stories/${storyId}/criteria`);
}

export function createCriterion(storyId: number, payload: Partial<AcceptanceCriteria>) {
  return pmHttp.post<AcceptanceCriteria>(
    `/v1/stories/${storyId}/criteria`,
    payload
  );
}

export function updateCriterion(id: number, payload: Partial<AcceptanceCriteria>) {
  return pmHttp.patch<AcceptanceCriteria>(`/v1/criteria/${id}`, payload);
}

export function checkCriterion(id: number, checked: boolean) {
  return pmHttp.post<AcceptanceCriteria>(`/v1/criteria/${id}/check`, { checked });
}

// ---------- tasks (Phase D4) ----------

export function listTasks(
  projectId: number,
  params: { page?: number; size?: number; status?: string; priority?: string; assigneeId?: number } = {}
) {
  return pmHttp.get<Paged<Task>>(
    `/v1/projects/${projectId}/tasks${qs(params)}`
  );
}

export function getTask(id: number) {
  return pmHttp.get<Task>(`/v1/tasks/${id}`);
}

export function createTask(projectId: number, payload: Partial<Task> & { title: string }) {
  return pmHttp.post<Task>(`/v1/projects/${projectId}/tasks`, payload);
}

export function updateTask(id: number, payload: Partial<Task>) {
  return pmHttp.patch<Task>(`/v1/tasks/${id}`, payload);
}

export function deleteTask(id: number) {
  return pmHttp.delete<void>(`/v1/tasks/${id}`);
}

// ---------- sprints (Phase D5) ----------

export interface Sprint {
  id: number;
  projectId: number;
  name: string;
  goal?: string;
  startDate?: string;
  endDate?: string;
  status: "planning" | "active" | "completed";
  createdBy: number;
  createdAt: string;
  updatedAt: string;
}

export function listSprints(
  projectId: number,
  params: { page?: number; size?: number; status?: string } = {}
) {
  return pmHttp.get<Paged<Sprint>>(`/v1/projects/${projectId}/sprints${qs(params)}`);
}

export function getSprint(id: number) {
  return pmHttp.get<Sprint>(`/v1/sprints/${id}`);
}

export function createSprint(projectId: number, payload: { name: string; goal?: string; status?: string; startDate?: string; endDate?: string }) {
  return pmHttp.post<Sprint>(`/v1/projects/${projectId}/sprints`, payload);
}

export function updateSprint(id: number, payload: Partial<Sprint>) {
  return pmHttp.patch<Sprint>(`/v1/sprints/${id}`, payload);
}

export function deleteSprint(id: number) {
  return pmHttp.delete<void>(`/v1/sprints/${id}`);
}
