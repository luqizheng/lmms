// 集中读取 e2e 环境变量，供 api / shared 复用
export const config = {
  baseURL: process.env.E2E_BASE_URL ?? 'http://localhost:3000',
  admin: {
    user: process.env.E2E_ADMIN_USER ?? 'e2eadmin',
    pass: process.env.E2E_ADMIN_PASS ?? 'P@ssw0rd!',
    nickname: 'E2E Admin',
    email: 'e2eadmin@test.local',
  },
  db: {
    host: process.env.E2E_DB_HOST ?? 'localhost',
    port: parseInt(process.env.E2E_DB_PORT ?? '3306', 10),
    user: process.env.E2E_DB_USER ?? 'root',
    password: process.env.E2E_DB_PASSWORD ?? 'rootpass',
    database: process.env.E2E_DB_NAME ?? 'superpowers',
  },
  // 基础数据：admin 账号用 username 标识（users.id 为 UUID，无法用数值上限区分）
  seedUsernames: (process.env.E2E_SEED_USERNAMES ?? 'e2eadmin').split(',').map((s) => s.trim()),
  debug: process.env.E2E_DEBUG === '1',
};
