import { readFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const cacheFile = join(__dirname, '.e2e-admin.json');

if (existsSync(cacheFile)) {
  const cache = JSON.parse(readFileSync(cacheFile, 'utf8')) as {
    token: string;
    username: string;
    password: string;
  };
  (globalThis as unknown as { __adminToken__: string }).__adminToken__ =
    cache.token;
  process.env.E2E_ADMIN_USER = cache.username;
  process.env.E2E_ADMIN_PASS = cache.password;
}
