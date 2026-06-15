const test = require('node:test');
const assert = require('node:assert/strict');
const { parseDockerfile } = require('../Base-Images-Scanner.js');

test('parseDockerfile extracts single FROM', () => {
  const content = 'FROM alpine:3.21\nRUN apk add curl\n';
  const refs = parseDockerfile(content);
  assert.deepEqual(refs, ['alpine:3.21']);
});

test('parseDockerfile extracts multi-stage FROM', () => {
  const content = [
    'FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder',
    'WORKDIR /app',
    'FROM alpine:3.21',
  ].join('\n');
  const refs = parseDockerfile(content);
  assert.deepEqual(refs, [
    'zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl',
    'alpine:3.21',
  ]);
});

test('parseDockerfile deduplicates identical refs', () => {
  const content = 'FROM alpine:3.21\nFROM alpine:3.21\n';
  const refs = parseDockerfile(content);
  assert.deepEqual(refs, ['alpine:3.21']);
});