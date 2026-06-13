const fs = require('node:fs');
const path = require('node:path');
const { execSync } = require('node:child_process');

const SERVICES_DIR = path.join(__dirname, '..', 'services');

function buildDocker() {
  const services = fs.readdirSync(SERVICES_DIR, { withFileTypes: true })
    .filter(dirent => dirent.isDirectory())
    .map(dirent => dirent.name);

  console.log('=== Docker 构建脚本 ===\n');

  for (const serviceName of services) {
    const serviceDir = path.join(SERVICES_DIR, serviceName);
    const dockerfile = path.join(serviceDir, 'Dockerfile');

    console.log(`\n## ${serviceName}`);

    if (!fs.existsSync(dockerfile)) {
      console.log('⚠️  缺失 Dockerfile，跳过构建');
      continue;
    }

    try {
      const command = `docker build -t ${serviceName}:latest ${serviceDir}`;
      console.log(`执行: ${command}`);
      execSync(command, { stdio: 'inherit' });
      console.log('✓ 构建成功');
    } catch (error) {
      console.log('✗ 构建失败');
    }
  }
}

buildDocker();