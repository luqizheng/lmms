import { login } from './helpers/auth';
import { SEED } from './fixtures/seed';

export default async function () {
  const token = await login(SEED.admin.user, SEED.admin.pass);
  (globalThis as unknown as { __adminToken__: string }).__adminToken__ = token;
}
