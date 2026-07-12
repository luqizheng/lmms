import axios, { type AxiosInstance } from "axios";
import { getToken, formatToken } from "@/utils/auth";

const pmHttp: AxiosInstance = axios.create({
  baseURL: import.meta.env.VITE_PM_API_BASE ?? "/pm-api",
  timeout: 120000, // LLM calls may be slow; 2 min
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

export interface CompletionResponse {
  provider: string;
  content: string;
  finishReason: string;
  disabled: boolean;
}

export interface DecomposedStory {
  title?: string;
  i_want?: string;
  so_that?: string;
  criteria?: Array<{ given?: string; when?: string; then?: string }>;
}

export interface DecomposedPayload {
  stories: DecomposedStory[];
}

export function listProviders() {
  return pmHttp.get<string[]>("/v1/llm/providers");
}

function qsProvider(provider?: string) {
  return provider ? `?provider=${encodeURIComponent(provider)}` : "";
}

export function decomposeRequirement(requirementId: number, provider?: string) {
  return pmHttp.post<CompletionResponse>(
    `/v1/llm/requirements/${requirementId}/decompose${qsProvider(provider)}`
  );
}

export function refineRequirement(requirementId: number, provider?: string) {
  return pmHttp.post<CompletionResponse>(
    `/v1/llm/requirements/${requirementId}/refine${qsProvider(provider)}`
  );
}

/**
 * Strip Markdown code fences (```json ... ``` or ``` ... ```) from an LLM
 * response and attempt to parse the result as the decompose JSON shape.
 * Returns null when parsing fails — callers should fall back to raw text.
 */
export function parseDecomposedJson(raw: string): DecomposedPayload | null {
  if (!raw) return null;
  let text = raw.trim();
  const fenceMatch = text.match(/^```(?:json)?\s*([\s\S]*?)```$/i);
  if (fenceMatch) text = fenceMatch[1].trim();
  try {
    const parsed = JSON.parse(text);
    if (parsed && Array.isArray(parsed.stories)) {
      return parsed as DecomposedPayload;
    }
    return null;
  } catch {
    return null;
  }
}
