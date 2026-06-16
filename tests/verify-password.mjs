import argon2 from 'argon2';

async function verifyPassword() {
  const password = 'admin123';
  const hash = '$argon2id$v=19$m=65536,t=3,p=4$jG5FyppNuhumCiyUVNRZZw$iKpLeMLY7Wfqtf0LncVhipNdj0ayt3RoS+YrUsct8SQ';
  
  try {
    const result = await argon2.verify(hash, password);
    console.log('Node.js argon2 verify result:', result);
  } catch (e) {
    console.error('Error:', e);
  }
}

verifyPassword();