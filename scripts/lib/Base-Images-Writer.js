'use strict';

const fs = require('node:fs');
const path = require('node:path');

const VERSION = 1;
const DEFAULT_REGISTRY = 'zhcoder-docker-registry.com:8000/official';

function mergeMirror(scanned, old, registry) {
  const reg = registry || DEFAULT_REGISTRY;
  const oldByRef = new Map();
  if (old && Array.isArray(old.images)) {
    for (const img of old.images) {
      if (img && img.ref) oldByRef.set(img.ref, img.mirror || null);
    }
  }
  return scanned.map((img) => {
    let mirror;
    if (img.mirror != null) {
      mirror = img.mirror;
    } else {
      const oldMirror = oldByRef.get(img.ref);
      if (oldMirror != null) {
        mirror = oldMirror;
      } else if (img.ref.startsWith('zhcoder-docker-registry.com:8000/')) {
        mirror = null;
      } else {
        mirror = reg;
      }
    }
    return { ref: img.ref, source: img.source, mirror };
  });
}

function writeManifest(outPath, images) {
  const data = {
    version: VERSION,
    generatedAt: new Date().toISOString(),
    images,
  };
  fs.mkdirSync(path.dirname(outPath), { recursive: true });
  fs.writeFileSync(outPath, JSON.stringify(data, null, 2) + '\n');
}

module.exports = { mergeMirror, writeManifest, DEFAULT_REGISTRY };