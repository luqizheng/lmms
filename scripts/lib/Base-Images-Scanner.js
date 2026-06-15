'use strict';

const FROM_REGEX = /^FROM\s+(\S+)/gm;

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

module.exports = { parseDockerfile };