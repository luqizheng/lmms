const http = require('http');
const { execSync } = require('child_process');
const { chalk, spinner, section } = require('./lib/ui.js');

const services = [
  { name: 'Frontend Dev Server', url: 'http://localhost:8849', type: 'http' },
  { name: 'API Gateway', url: 'http://localhost:3000/health', type: 'http' },
  { name: 'Token Service', url: 'http://localhost:3000/.well-known/jwks.json', type: 'http' },
  { name: 'IAM Service', url: 'http://localhost:3000/v1/iam/health', type: 'http' },
  { name: 'Notification Service', url: 'http://localhost:3000/v1/notification/health', type: 'http' },
  { name: 'Prue Backend', url: 'http://localhost:3000/v1/pure/health', type: 'http' },
  { name: 'Consul', url: 'http://localhost:8500/v1/health/node/consul', type: 'http' },
];

const dockerServices = [
  { name: 'consul', expectedStatus: 'healthy' },
  { name: 'redis', expectedStatus: 'healthy' },
  { name: 'mysql', expectedStatus: 'healthy' },
  { name: 'token-service', expectedStatus: 'healthy' },
  { name: 'iam-service', expectedStatus: 'running' },
  { name: 'notification-service', expectedStatus: 'running' },
  { name: 'prue_backend', expectedStatus: 'running' },
  { name: 'api-gateway', expectedStatus: 'running' },
];

function checkHttpService(service) {
  return new Promise((resolve) => {
    const options = new URL(service.url);
    const req = http.request(options, (res) => {
      resolve({ ...service, status: 'success', code: res.statusCode });
    });
    req.on('error', () => {
      resolve({ ...service, status: 'failed', code: null });
    });
    req.setTimeout(3000, () => {
      resolve({ ...service, status: 'timeout', code: null });
      req.abort();
    });
    req.end();
  });
}

function checkDockerServices() {
  try {
    const output = execSync('docker-compose -f infrastructure/docker-compose.yml ps --format "{{.Service}}|{{.State}}"', {
      encoding: 'utf-8',
      stdio: ['pipe', 'pipe', 'ignore']
    });
    const lines = output.trim().split('\n');
    const statusMap = {};
    lines.forEach(line => {
      const parts = line.split('|');
      if (parts.length >= 2) {
        const service = parts[0].trim();
        const state = parts[1].trim().toLowerCase();
        statusMap[service] = state;
      }
    });
    return dockerServices.map(service => {
      const actualStatus = statusMap[service.name] || 'not found';
      const isHealthy = actualStatus.includes('healthy');
      const isRunning = actualStatus.includes('running');
      let status = 'failed';
      if (service.expectedStatus === 'healthy' && (isHealthy || isRunning)) {
        status = 'success';
      } else if (service.expectedStatus === 'running' && (isRunning || isHealthy)) {
        status = 'success';
      }
      return { ...service, actualStatus: isHealthy ? 'healthy' : (isRunning ? 'running' : actualStatus), status };
    });
  } catch (err) {
    return dockerServices.map(service => ({
      ...service,
      actualStatus: 'error',
      status: 'failed'
    }));
  }
}

function formatMark(status) {
  return status === 'success' ? chalk.green('✓') : chalk.red('✗');
}

async function runChecks() {
  console.log(chalk.bold.cyan('=== Development Environment Check ==='));

  section('Docker Services');
  const dockerSp = spinner('inspecting docker compose ps ...').start();
  const dockerResults = checkDockerServices();
  dockerSp.stop();
  dockerResults.forEach(result => {
    console.log(`  ${formatMark(result.status)} ${result.name}: ${result.actualStatus} ${chalk.gray(`(expected: ${result.expectedStatus})`)}`);
  });

  section('HTTP Services');
  const httpSp = spinner(`probing ${services.length} endpoints ...`).start();
  const httpResults = await Promise.all(services.map(checkHttpService));
  httpSp.stop();
  httpResults.forEach(result => {
    const statusText = result.status === 'success'
      ? `HTTP ${result.code}`
      : result.status === 'timeout'
        ? 'Timeout'
        : 'Unreachable';
    console.log(`  ${formatMark(result.status)} ${result.name}: ${statusText}`);
  });

  const allSuccess = [...dockerResults, ...httpResults].every(r => r.status === 'success');
  console.log('');
  if (allSuccess) {
    console.log(chalk.green('✓ 全部服务健康'));
  } else {
    console.log(chalk.red('✗ 有服务未就绪，见上'));
  }
  process.exit(allSuccess ? 0 : 1);
}

runChecks().catch(err => {
  console.error(chalk.red('Error during check:'), err);
  process.exit(1);
});
