export const Payload = {
  register: () => ({
    username: `e2e_reg_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`,
    password: 'P@ssw0rd!',
    email: `e2e_reg_${Date.now()}@test.local`,
  }),
  token: {
    access: () => ({
      grant_type: 'authorization_code',
      code: 'placeholder',
      client_id: process.env.E2E_CLIENT_ID ?? 'e2e-client',
      client_secret: process.env.E2E_CLIENT_SECRET ?? 'e2e-secret',
    }),
    refresh: (rt: string) => ({
      refresh_token: rt,
      client_id: process.env.E2E_CLIENT_ID ?? 'e2e-client',
      client_secret: process.env.E2E_CLIENT_SECRET ?? 'e2e-secret',
    }),
    apiKey: () => ({ name: `e2e_key_${Date.now()}`, scopes: ['read'] }),
  },
  role: () => ({
    name: `e2e_role_${Date.now()}`,
    code: `E2E_${Date.now()}`,
    description: 'created by e2e',
  }),
};
