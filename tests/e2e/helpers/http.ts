import request from 'supertest';

const baseURL = process.env.E2E_BASE_URL ?? 'http://localhost:8080';

function setAuth(agent: request.Test, token: string): request.Test {
  return agent.set('Authorization', `Bearer ${token}`);
}

export function authedReq(token: string) {
  return {
    get: (path: string) =>
      setAuth(request(baseURL).get(path), token),
    post: (path: string, data?: unknown) =>
      setAuth(request(baseURL).post(path), token).send(data ?? {}),
    put: (path: string, data?: unknown) =>
      setAuth(request(baseURL).put(path), token).send(data ?? {}),
    patch: (path: string, data?: unknown) =>
      setAuth(request(baseURL).patch(path), token).send(data ?? {}),
    del: (path: string, data?: unknown) =>
      setAuth(request(baseURL).delete(path), token).send(data ?? {}),
  };
}

export { baseURL };
