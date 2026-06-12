import { describe, it, expect } from 'vitest';
import { authedReq } from './http';


describe('authedReq', () => {
  const baseURL = process.env.E2E_BASE_URL ?? 'http://localhost:3000';

  it('should build a request helper', async () => {
    const r = authedReq('fake-token');
    expect(r).toBeDefined();
    expect(typeof r.get).toBe('function');
    expect(typeof r.post).toBe('function');
    expect(typeof r.put).toBe('function');
    expect(typeof r.patch).toBe('function');
    expect(typeof r.del).toBe('function');
    expect(baseURL).toMatch(/^https?:\/\//);
  });
});
