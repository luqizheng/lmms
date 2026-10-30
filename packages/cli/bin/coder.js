#!/usr/bin/env node
'use strict';

const { run } = require('../lib/cli.js');

run(process.argv.slice(2))
  .then((code) => { process.exitCode = code; })
  .catch((err) => {
    process.stderr.write(err && err.stack ? err.stack + '\n' : String(err) + '\n');
    process.exitCode = 1;
  });
