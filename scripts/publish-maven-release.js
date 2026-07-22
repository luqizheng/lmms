#!/usr/bin/env node
'use strict';

const { runPublish } = require('./lib/maven-publish.js');

runPublish('release', process.argv);
