import { config } from './config';

const TIMEOUT_MS = 3000;

interface Probe {
  name: string;
  url: string;
  expectStatus?: number;
}

const PROBES: Probe[] = [
  { name: 'api-gateway /health', url: `${config.baseURL}/health` },
  { name: 'iam via gateway', url: `${config.baseURL}/iam/health`, expectStatus: 200 },
];

export async function preflight(): Promise<void> {
  for (const probe of PROBES) {
    try {
      const res = await fetch(probe.url, {
        signal: AbortSignal.timeout(TIMEOUT_MS),
      });
      const expected = probe.expectStatus ?? 200;
      if (!res.ok && res.status !== expected) {
        throw new Error(
          `preflight ${probe.name} → ${res.status} (期望 ${expected})`,
        );
      }
      console.log(`[preflight] OK ${probe.name} (${res.status})`);
    } catch (e) {
      throw new Error(
        `preflight ${probe.name} FAILED: ${(e as Error).message}\n` +
          `  → 请先启动 infrastructure: docker compose -f infrastructure/docker-compose.yml up -d`,
      );
    }
  }
}