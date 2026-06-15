'use strict';

const fs = require('node:fs');
const path = require('node:path');

const VERSION = 1;

function mergeMirror(scanned, old) {
  const oldByRef = new Map();
  if (old && Array.isArray(old.images)) {
    for (const img of old.images) {
      if (img && img.ref) oldByRef.set(img.ref, img.mirror || null);
    }
  }
  return scanned.map((img) => {
    const oldMirror = oldByRef.get(img.ref);
    return {
      ref: img.ref,
      source: img.source,
      mirror: img.mirror != null ? img.mirror : (oldMirror != null ? oldMirror : null),
    };
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

module.exports = { mergeMirror, writeManifest };