export const SEED = {
  admin: {
    user: process.env.E2E_ADMIN_USER ?? 'admin',
    pass: process.env.E2E_ADMIN_PASS ?? 'admin123',
  },
  newUser: () => ({
    username: `e2e_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
    password: 'P@ssw0rd!',
    email: `e2e_${Date.now()}@test.local`,
    nickname: 'E2E User',
  }),
  newRole: () => ({
    name: `e2e_role_${Date.now()}`,
    code: `E2E_${Date.now()}`,
    description: 'created by e2e',
  }),
};
