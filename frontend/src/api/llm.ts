import axios, { type AxiosInstance } from "axios";
import { getToken, formatToken } from "@/utils/auth";

const pmHttp: AxiosInstance = axios.create({
  baseURL: import.meta.env.VITE_PM_API_BASE ?? "/pm-api",
  timeout: 30000,
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

export function listProviders() {
  return pmHttp.get<string[]>("/v1/llm/providers");
}

export function decomposeRequirement(requirementId: number) {
  return pmHttp.post<CompletionResponse>(
    `/v1/llm/requirements/${requirementId}/decompose`
  );
}

export function refineRequirement(requirementId: number) {
  return pmHttp.post<CompletionResponse>(
    `/v1/llm/requirements/${requirementId}/refine`
  );
}
