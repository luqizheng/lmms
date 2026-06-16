import http from 'http';

async function testLogin() {
  // 测试1: 尝试登录
  const data = JSON.stringify({ 
    username: 'admin', 
    password: 'admin123'
  });

  const req = http.request({
    hostname: 'localhost',
    port: 6002,
    path: '/iam/auth/login',
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Content-Length': data.length
    }
  }, (res) => {
    console.log('测试1 - 登录:');
    console.log('Status:', res.statusCode);
    let body = '';
    res.on('data', (chunk) => body += chunk);
    res.on('end', () => {
      console.log('Body:', body);
      console.log('');
      
      // 测试2: 尝试创建新用户
      createUser();
    });
  });

  req.on('error', (e) => console.error('Error:', e));
  req.write(data);
  req.end();
}

function createUser() {
  const data = JSON.stringify({ 
    username: 'testuser_new', 
    password: 'testpass123',
    nickname: 'Test User',
    email: 'test@example.com',
    role_ids: []
  });

  const req = http.request({
    hostname: 'localhost',
    port: 6002,
    path: '/iam/users',
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Content-Length': data.length
    }
  }, (res) => {
    console.log('测试2 - 创建用户:');
    console.log('Status:', res.statusCode);
    let body = '';
    res.on('data', (chunk) => body += chunk);
    res.on('end', () => {
      console.log('Body:', body);
    });
  });

  req.on('error', (e) => console.error('Error:', e));
  req.write(data);
  req.end();
}

testLogin();