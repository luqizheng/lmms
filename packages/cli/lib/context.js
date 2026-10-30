'use strict';

const fs = require('node:fs');
const path = require('node:path');

// Context markers (spec 裁决 #8, three contexts; downstream rules land in batch 2):
//   monorepo: Cargo.toml + services/ + libs/
//   dist:     images/manifest.json + docker-compose.yml + Makefile
function detectContext(startDir) {
  let dir = path.resolve(startDir);
  for (;;) {
    if (
      fs.existsSync(path.join(dir, 'Cargo.toml')) &&
      fs.existsSync(path.join(dir, 'services')) &&
      fs.existsSync(path.join(dir, 'libs'))
    ) {
      return { kind: 'monorepo', root: dir };
    }
    if (
      fs.existsSync(path.join(dir, 'images', 'manifest.json')) &&
      fs.existsSync(path.join(dir, 'docker-compose.yml')) &&
      fs.existsSync(path.join(dir, 'Makefile'))
    ) {
      return { kind: 'dist', root: dir };
    }
    const parent = path.dirname(dir);
    if (parent === dir) return { kind: 'unknown', root: null };
    dir = parent;
  }
}

module.exports = { detectContext };
