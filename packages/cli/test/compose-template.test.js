'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { collectTemplateFiles, buildComposeTemplate } = require('../lib/compose-template.js');

function fixture() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'coder-tpl-'));
  const files = [
    'docker-compose.yml',
    'docker-compose-infra.yml',
    'docker-compose-services.yml',
    'base-images.json',
    '.env.prod.example',
    'README.md',
    'config/prometheus/prometheus.yml',
    'config/rabbitmq/rabbitmq.conf',
    'docker/healthcheck.sh',
    'mysql-replica/Dockerfile',
    'scripts/deploy-remote.sh',
    // excluded
    '.env',
    'AGENTS.md',
    'STATE.md',
    'LOOP.md',
    'loop-budget.md',
    'loop-run-log.md',
    '.grok/session.json',
    'vendor/swagger-ui.zip',
  ];
  for (const rel of files) {
    const p = path.join(dir, rel);
    fs.mkdirSync(path.dirname(p), { recursive: true });
    fs.writeFileSync(p, 'x', 'utf-8');
  }
  return dir;
}

test('collectTemplateFiles keeps compose layout and excludes secrets/session files', () => {
  const dir = fixture();
  const files = collectTemplateFiles(dir);
  const rel = files.map((f) => f.rel).sort();
  assert.ok(rel.includes('docker-compose.yml'));
  assert.ok(rel.includes('docker-compose-infra.yml'));
  assert.ok(rel.includes('base-images.json'));
  assert.ok(rel.includes('.env.prod.example'), 'whitelisted dotfile kept');
  assert.ok(rel.includes('README.md'));
  assert.ok(rel.includes(path.join('config', 'prometheus', 'prometheus.yml').replaceAll('\\', '/')));
  assert.ok(rel.includes(path.join('scripts', 'deploy-remote.sh').replaceAll('\\', '/')));
  assert.ok(!rel.includes('.env'), 'real secrets excluded');
  assert.ok(!rel.includes('AGENTS.md'));
  assert.ok(!rel.includes('STATE.md'));
  assert.ok(!rel.includes('LOOP.md'));
  assert.ok(!rel.some((f) => /^loop-/.test(f)), 'loop notes excluded');
  assert.ok(!rel.some((f) => f.startsWith('.grok')), 'dot dirs excluded');
  assert.ok(!rel.some((f) => f.startsWith('vendor')), 'build-time vendor excluded');
});

test('buildComposeTemplate copies files and writes TEMPLATE_INFO.json', () => {
  const src = fixture();
  const outRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'coder-tpl-out-'));
  const result = buildComposeTemplate({ infraDir: src, outRoot, version: '1.6.0' });

  assert.match(path.basename(result.dir), /coder-compose-template-1\.6\.0$/);
  const copied = fs.readFileSync(path.join(result.dir, 'docker-compose.yml'), 'utf-8');
  assert.equal(copied, 'x');
  assert.ok(fs.existsSync(path.join(result.dir, 'config', 'rabbitmq', 'rabbitmq.conf')));
  assert.ok(!fs.existsSync(path.join(result.dir, '.env')), 'secret never lands in the bundle');

  const info = JSON.parse(fs.readFileSync(path.join(result.dir, 'TEMPLATE_INFO.json'), 'utf-8'));
  assert.equal(info.version, '1.6.0');
  assert.equal(info.fileCount, result.fileCount);
  assert.ok(info.generatedAt);
  assert.ok(result.fileCount >= 8);
});

test('buildComposeTemplate overwrites a previous bundle of the same version', () => {
  const src = fixture();
  const outRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'coder-tpl-out2-'));
  buildComposeTemplate({ infraDir: src, outRoot, version: '1.6.0' });
  const second = buildComposeTemplate({ infraDir: src, outRoot, version: '1.6.0' });
  assert.ok(fs.existsSync(path.join(second.dir, 'TEMPLATE_INFO.json')));
});
