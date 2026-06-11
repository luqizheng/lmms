import request from 'supertest';
import { baseURL } from './http';
import { logReq, logRes } from './debug';

export interface LoginRes {
  access_token: string;
  refresh_token?: string;
  token_type?: string;
  expires_in?: number;
}

export async function login(username: string, password: string): Promise<string> {
  const path = '/api/v1/auth/login';
  logReq('POST', path, { username });
  const res = await request(baseURL).post(path).send({ username, password });
  logRes(res.status, res.body);
  if (res.status !== 200) {
    throw new Error(`login failed: ${res.status} ${JSON.stringify(res.body)}`);
  }
  const body = res.body as LoginRes;
  if (!body.access_token) {
    throw new Error(`login response missing access_token: ${JSON.stringify(body)}`);
  }
  return body.access_token;
}

export async function logout(token: string): Promise<void> {
  const path = '/api/v1/auth/logout';
  logReq('POST', path);
  const res = await request(baseURL)
    .post(path)
    .set('Authorization', `Bearer ${token}`);
  logRes(res.status, res.body);
}
