const fs = require('node:fs');
const path = require('node:path');

const SERVICES_DIR = path.join(__dirname, '..', 'services');

function checkServiceStandard() {
  const services = fs.readdirSync(SERVICES_DIR, { withFileTypes: true })
    .filter(dirent => dirent.isDirectory())
    .map(dirent => dirent.name);

  console.log('=== 服务规范检查报告 ===\n');

  for (const serviceName of services) {
    const serviceDir = path.join(SERVICES_DIR, serviceName);
    const rulesFile = path.join(serviceDir, 'AGENTS.md');

    console.log(`\n## ${serviceName}`);

    if (!fs.existsSync(rulesFile)) {
      console.log('⚠️  缺失规则文件: AGENTS.md');
      continue;
    }

    const rulesContent = fs.readFileSync(rulesFile, 'utf-8');
    const requiredComponents = parseRequiredComponents(rulesContent);

    for (const component of requiredComponents) {
      const componentPath = getComponentPath(serviceDir, component);
      const exists = fs.existsSync(componentPath);

      if (exists) {
        console.log(`✓ ${component}`);
      } else {
        console.log(`⚠️  缺失: ${component}`);
      }
    }
  }
}

function parseRequiredComponents(content) {
  const match = content.match(/## 必需组件\n([\s\S]*?)\n##/);
  if (!match) return [];

  return match[1]
    .split('\n')
    .filter(line => line.startsWith('-'))
    .map(line => line.replace('- ', '').trim());
}

function getComponentPath(serviceDir, component) {
  const componentMap = {
    'Dockerfile': 'Dockerfile',
    'Swagger 配置': 'config/swagger.yaml',
    '健康检查端点': 'src/health.rs',
    '数据库迁移文件': 'migrations',
    'README.md': 'README.md',
    'tests/ 目录': 'tests'
  };

  return path.join(serviceDir, componentMap[component] || component);
}

checkServiceStandard();