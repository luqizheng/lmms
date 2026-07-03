const http = require('http');
const { execSync } = require('child_process');

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

async function runChecks() {
  console.log('=== Development Environment Check ===\n');

  console.log('[Docker Services Status]');
  const dockerResults = checkDockerServices();
  dockerResults.forEach(result => {
    const symbol = result.status === 'success' ? 'OK' : 'FAIL';
    const color = result.status === 'success' ? '\x1b[32m' : '\x1b[31m';
    const reset = '\x1b[0m';
    console.log(`  ${color}${symbol}${reset} ${result.name}: ${result.actualStatus} (expected: ${result.expectedStatus})`);
  });

  console.log('\n[HTTP Services Status]');
  const httpResults = await Promise.all(services.map(checkHttpService));
  httpResults.forEach(result => {
    const symbol = result.status === 'success' ? 'OK' : 'FAIL';
    const color = result.status === 'success' ? '\x1b[32m' : '\x1b[31m';
    const reset = '\x1b[0m';
    const statusText = result.status === 'success' 
      ? `HTTP ${result.code}` 
      : result.status === 'timeout' 
        ? 'Timeout' 
        : 'Unreachable';
    console.log(`  ${color}${symbol}${reset} ${result.name}: ${statusText}`);
  });

  console.log('\n=== Check Complete ===');
  const allSuccess = [...dockerResults, ...httpResults].every(r => r.status === 'success');
  process.exit(allSuccess ? 0 : 1);
}

runChecks().catch(err => {
  console.error('Error during check:', err);
  process.exit(1);
});