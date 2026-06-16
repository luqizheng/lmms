import { login } from './helpers/auth';
import { SEED } from './fixtures/seed';
import request from 'supertest';
import { baseURL } from './helpers/http';
import { clearUsers } from './helpers/db';

export default async function () {
  await clearUsers();
}