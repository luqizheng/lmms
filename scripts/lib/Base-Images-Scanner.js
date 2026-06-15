'use strict';

const FROM_REGEX = /^FROM\s+(\S+)/gm;
const COMPOSE_IMAGE_REGEX = /^\s*image:\s*(\S+)/gm;

function parseDockerfile(content) {
  const refs = [];
  const seen = new Set();
  for (const m of content.matchAll(FROM_REGEX)) {
    const ref = m[1];
    if (ref.startsWith('--')) continue;
    const stage = ref.split(/\s+/)[0];
    if (!seen.has(stage)) {
      seen.add(stage);
      refs.push(stage);
    }
  }
  return refs;
}

function parseCompose(content) {
  const refs = [];
  const seen = new Set();
  for (const m of content.matchAll(COMPOSE_IMAGE_REGEX)) {
    const ref = m[1].replace(/['"]/g, '');
    if (seen.has(ref)) continue;
    seen.add(ref);
    refs.push(ref);
  }
  return refs;
}

module.exports = { parseDockerfile, parseCompose };