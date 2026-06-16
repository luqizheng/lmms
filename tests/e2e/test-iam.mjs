import http from 'http';

const data = JSON.stringify({ 
  username: 'testuser456', 
  password: 'testpass123',
  nickname: 'Test User',
  email: 'test@example.com',
  role_ids: []
});

const req = http.request({
  hostname: 'localhost',
  port: 3000,
  path: '/iam/auth/register',
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'Content-Length': data.length
  }
}, (res) => {
  console.log('Status:', res.statusCode);
  let body = '';
  res.on('data', (chunk) => body += chunk);
  res.on('end', () => console.log('Body:', body));
});

req.on('error', (e) => console.error('Error:', e));
req.write(data);
req.end();