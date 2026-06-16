import { createConnection } from 'mysql2/promise';

const DB_CONFIG = {
  host: process.env.E2E_DB_HOST ?? 'localhost',
  port: parseInt(process.env.E2E_DB_PORT ?? '3306', 10),
  user: process.env.E2E_DB_USER ?? 'root',
  password: process.env.E2E_DB_PASSWORD ?? 'rootpass',
  database: process.env.E2E_DB_NAME ?? 'superpowers',
};

export async function queryUserByUsername(username: string): Promise<{ id: string; username: string; email: string; nickname: string } | null> {
  const connection = await createConnection(DB_CONFIG);
  try {
    const [rows] = await connection.execute(
      'SELECT id, username, email, nickname FROM users WHERE username = ?',
      [username]
    );
    const result = rows as Array<{ id: string; username: string; email: string; nickname: string }>;
    return result.length > 0 ? result[0] : null;
  } finally {
    await connection.end();
  }
}

export async function queryUserById(userId: string): Promise<{ id: string; username: string; email: string; nickname: string } | null> {
  const connection = await createConnection(DB_CONFIG);
  try {
    const [rows] = await connection.execute(
      'SELECT id, username, email, nickname FROM users WHERE id = ?',
      [userId]
    );
    const result = rows as Array<{ id: string; username: string; email: string; nickname: string }>;
    return result.length > 0 ? result[0] : null;
  } finally {
    await connection.end();
  }
}