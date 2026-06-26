#!/usr/bin/env node
/**
 * 从 contracts/events.yaml 生成 Rust 事件 struct + routing-key 常量
 * 输出：
 *   - libs/common-event/src/routing_keys.rs
 *   - libs/common-event/src/events.rs （占位，Task 7 替换为带 trait impl 的版本）
 */

const fs = require('fs');
const path = require('path');

const YAML = require('js-yaml');
const yamlPath = path.resolve(__dirname, '../events.yaml');
const outDir = path.resolve(__dirname, '../../libs/common-event/src');
fs.mkdirSync(outDir, { recursive: true });

const doc = YAML.load(fs.readFileSync(yamlPath, 'utf8'));
const events = doc.events || {};
const defaultVersion = doc.default_schema_version || 1;

// ---- routing_keys.rs ----
const constLines = Object.keys(events).map((rk) => {
  const NAME = rk.toUpperCase().replace(/\./g, '_');
  return `pub const ${NAME}: &str = "${rk}";`;
});
const routingKeysRs = [
  '// Auto-generated from contracts/events.yaml',
  '// DO NOT EDIT MANUALLY',
  '',
  '//! Routing-key constants for events published to the `events` topic exchange.',
  '',
  ...constLines,
  '',
].join('\n');
fs.writeFileSync(path.join(outDir, 'routing_keys.rs'), routingKeysRs);

// ---- events.rs ----
const pascal = (rk) => rk.split('.').map((s) => s[0].toUpperCase() + s.slice(1)).join('');

const structDefs = Object.entries(events).map(([rk, def]) => {
  const NAME = pascal(rk);
  const version = def.schema_version || defaultVersion;
  const fields = Object.entries(def.payload || {}).map(([fname, fdef]) => {
    const T = rustType(fdef.type);
    const OPT = fdef.required ? '' : 'Option<';
    const CLOSE = fdef.required ? '' : '>';
    return `    pub ${fname}: ${OPT}${T}${CLOSE},`;
  }).join('\n');
  return [
    `#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]`,
    `pub struct ${NAME} {`,
    fields,
    `}`,
    ``,
    `impl Event for ${NAME} {`,
    `    fn event_type() -> &'static str { "${rk}" }`,
    `    fn schema_version() -> u32 { ${version} }`,
    `}`,
  ].join('\n');
});

const enumVariants = Object.keys(events).map((rk) => {
  const NAME = pascal(rk);
  return `    ${NAME}(Envelope<${NAME}>),`;
}).join('\n');

const eventsRs = [
  '// Auto-generated from contracts/events.yaml',
  '// DO NOT EDIT MANUALLY',
  '',
  'use serde::{Deserialize, Serialize};',
  '',
  'use crate::envelope::Envelope;',
  'use crate::event::Event;',
  '',
  ...structDefs,
  '',
  '#[derive(Debug, Clone, Serialize, Deserialize)]',
  '#[serde(tag = "event_type", content = "payload", rename_all = "snake_case")]',
  'pub enum IamEventEnum {',
  `    Unknown {`,
  `        event_type: String,`,
  `        envelope: serde_json::Value,`,
  `    },`,
  enumVariants,
  '}',
  '',
].join('\n');
fs.writeFileSync(path.join(outDir, 'events.rs'), eventsRs);

function rustType(t) {
  return ({ string: 'String', int: 'i64', number: 'f64', bool: 'bool' })[t] || 'serde_json::Value';
}

console.log(`Generated ${Object.keys(events).length} event structs into ${outDir}`);
