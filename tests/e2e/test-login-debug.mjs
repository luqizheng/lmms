import http from 'http';

const data = JSON.stringify({ 
  username: 'admin', 
  password: 'admin123'
});

const req = http.request({
  hostname: 'localhost',
  port: 3000,
  path: '/api/v1/auth/login',
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'Content-Length': data.length
  }
}, (res) => {
  console.log('Status:', res.statusCode);
  let body = '';
  res.on('data', (chunk) => body += chunk);
  res.on('end', () => {
    console.log('Body:', body);
    console.log('Headers:', JSON.stringify(res.headers));
  });
});

req.on('error', (e) => console.error('Error:', e));
req.write(data);
req.end();