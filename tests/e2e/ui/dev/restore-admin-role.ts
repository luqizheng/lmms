// 开发期工具：并发 agent 的 runner 会在 beforeAll 调 resetBusiness 清空 iam_user_roles，
// 导致 e2eadmin 丢失 tenant_admin 角色（登录 JWT permissions 为空 → 全线 403）。
// 本脚本按设计基线（首用户=租户管理员）补回该行，幂等，可随时重跑。
import './env';
import { createConnection } from 'mysql2/promise';
import { config } from '../../shared/config';

const conn = await createConnection(config.db);
try {
  const [users] = await conn.query(
    `SELECT id FROM iam_users WHERE username = ? AND deleted_at IS NULL`,
    [config.admin.user],
  );
  const userId = (users as { id: string }[])[0]?.id;
  if (!userId) throw new Error(`用户不存在: ${config.admin.user}`);

  const [roles] = await conn.query(
    `SELECT id FROM iam_roles WHERE code = 'tenant_admin' AND deleted_at IS NULL`,
  );
  const roleId = (roles as { id: string }[])[0]?.id;
  if (!roleId) throw new Error('tenant_admin 角色不存在');

  const [result] = await conn.query(
    `INSERT IGNORE INTO iam_user_roles (user_id, role_id, created_at) VALUES (?, ?, NOW())`,
    [userId, roleId],
  );
  console.log(
    `RESTORED user=${config.admin.user}(${userId}) role=tenant_admin(${roleId}) affected=${(result as { affectedRows: number }).affectedRows}`,
  );
} finally {
  await conn.end();
}
