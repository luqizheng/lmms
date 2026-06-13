const fs = require('node:fs');
const path = require('node:path');
const { execSync } = require('node:child_process');

const SERVICES_DIR = path.join(__dirname, '..', 'services');

function buildCargo() {
  const services = fs.readdirSync(SERVICES_DIR, { withFileTypes: true })
    .filter(dirent => dirent.isDirectory())
    .map(dirent => dirent.name);

  console.log('=== Cargo 构建脚本 ===\n');

  for (const serviceName of services) {
    const serviceDir = path.join(SERVICES_DIR, serviceName);
    const cargoToml = path.join(serviceDir, 'Cargo.toml');

    console.log(`\n## ${serviceName}`);

    if (!fs.existsSync(cargoToml)) {
      console.log('⚠️  缺失 Cargo.toml，跳过构建');
      continue;
    }

    try {
      const command = `cargo build --release -p ${serviceName}`;
      console.log(`执行: ${command}`);
      execSync(command, { stdio: 'inherit', cwd: path.join(__dirname, '..') });
      console.log('✓ 构建成功');
    } catch (error) {
      console.log('✗ 构建失败');
    }
  }
}

buildCargo();