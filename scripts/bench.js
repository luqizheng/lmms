/**
 * 性能压测脚本
 * 
 * 使用: node scripts/bench.js [service] [duration] [concurrency]
 * 
 * 示例:
 *   node scripts/bench.js audit-service 60 10
 */

const http = require('http');
const https = require('https');

const SERVICES = {
  'audit-service': 'http://localhost:8080',
  'config-service': 'http://localhost:8081',
  'file-service': 'http://localhost:8082',
};

class LoadTester {
  constructor(url, duration, concurrency) {
    this.url = url;
    this.duration = duration * 1000;
    this.concurrency = concurrency;
    this.results = {
      requests: 0,
      successes: 0,
      errors: 0,
      timeouts: 0,
      latencies: [],
    };
    this.startTime = null;
    this.running = true;
  }

  async request() {
    const start = Date.now();
    return new Promise((resolve) => {
      const protocol = this.url.startsWith('https') ? https : http;
      const req = protocol.get(this.url + '/health', (res) => {
        const latency = Date.now() - start;
        this.results.requests++;
        this.results.latencies.push(latency);
        
        if (res.statusCode < 400) {
          this.results.successes++;
        } else {
          this.results.errors++;
        }
        resolve();
      });

      req.on('error', () => {
        this.results.requests++;
        this.results.errors++;
        resolve();
      });

      req.setTimeout(5000, () => {
        this.results.requests++;
        this.results.timeouts++;
        req.destroy();
        resolve();
      });
    });
  }

  async worker() {
    while (this.running && Date.now() - this.startTime < this.duration) {
      await this.request();
    }
  }

  async run() {
    console.log(`\nStarting load test:`);
    console.log(`  URL: ${this.url}`);
    console.log(`  Duration: ${this.duration / 1000}s`);
    console.log(`  Concurrency: ${this.concurrency}\n`);

    this.startTime = Date.now();
    const workers = Array(this.concurrency).fill().map(() => this.worker());
    await Promise.all(workers);
    
    return this.report();
  }

  stop() {
    this.running = false;
  }

  report() {
    const elapsed = (Date.now() - this.startTime) / 1000;
    const rps = this.results.requests / elapsed;
    
    const sorted = [...this.results.latencies].sort((a, b) => a - b);
    const p50 = sorted[Math.floor(sorted.length * 0.5)] || 0;
    const p90 = sorted[Math.floor(sorted.length * 0.9)] || 0;
    const p99 = sorted[Math.floor(sorted.length * 0.99)] || 0;
    const avg = sorted.length > 0 
      ? sorted.reduce((a, b) => a + b, 0) / sorted.length 
      : 0;
    const max = sorted[sorted.length - 1] || 0;

    return {
      elapsed,
      rps,
      total: this.results.requests,
      successes: this.results.successes,
      errors: this.results.errors,
      timeouts: this.results.timeouts,
      latencies: { avg, p50, p90, p99, max },
    };
  }
}

function printReport(report) {
  console.log('\n=== Load Test Results ===');
  console.log(`Duration:    ${report.elapsed.toFixed(2)}s`);
  console.log(`Total Req:  ${report.total}`);
  console.log(`RPS:        ${report.rps.toFixed(2)}`);
  console.log(`\nResponses:`);
  console.log(`  Success:  ${report.successes}`);
  console.log(`  Errors:   ${report.errors}`);
  console.log(`  Timeouts: ${report.timeouts}`);
  console.log(`\nLatency (ms):`);
  console.log(`  Avg:      ${report.latencies.avg.toFixed(2)}`);
  console.log(`  P50:      ${report.latencies.p50}`);
  console.log(`  P90:      ${report.latencies.p90}`);
  console.log(`  P99:      ${report.latencies.p99}`);
  console.log(`  Max:      ${report.latencies.max}`);
}

async function main() {
  const service = process.argv[2] || 'audit-service';
  const duration = parseInt(process.argv[3]) || 30;
  const concurrency = parseInt(process.argv[4]) || 10;

  const url = SERVICES[service];
  if (!url) {
    console.error(`Unknown service: ${service}`);
    console.error(`Available: ${Object.keys(SERVICES).join(', ')}`);
    process.exit(1);
  }

  const tester = new LoadTester(url, duration, concurrency);
  
  process.on('SIGINT', () => {
    console.log('\n\nStopping...');
    tester.stop();
  });

  try {
    const report = await tester.run();
    printReport(report);
  } catch (err) {
    console.error('Error:', err.message);
    process.exit(1);
  }
}

main();
