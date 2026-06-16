import argon2 from 'argon2';
import fs from 'fs';

async function createTestUser() {
  const password = 'admin123';
  const hash = await argon2.hash(password);
  console.log('Generated hash:', hash);
  
  const sql = `INSERT INTO users (id, username, password_hash, salt, nickname, email, phone, avatar, status, email_verified, phone_verified, created_at, updated_at) VALUES ('test-admin-1', 'admin', '${hash}', 'test-salt-123', 'Admin User', 'admin@example.com', NULL, NULL, 'active', false, false, NOW(), NOW());`;
  
  fs.writeFileSync('tests/insert-admin.sql', sql);
  console.log('SQL file created');
}

createTestUser().catch(e => console.error(e));