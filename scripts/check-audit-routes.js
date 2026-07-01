#!/usr/bin/env node
/**
 * Mirror of services/api-gateway/src/template.rs::{extract,render}
 *   用于在没有 cargo 环境时验证路由模板与请求路径的匹配。
 *
 * 业务背景：audit.router.toml 中
 *   upstream   = "/v1/audit/export/{rest}"
 *   downstream = "/audit/export/{rest}"
 *   当请求为 /v1/audit/export/compliance/operation/csv 时，
 *   需确认 {rest} 能吞多段并正确转发到 downstream。
 *
 * 调用：node scripts/check-audit-routes.js
 *   退出 0：路由可吞多段；退出 1：需补 {rest:.*} 通配或拆分。
 */

function parse(input) {
  const parts = [];
  let buf = "";
  let seen = new Set();
  for (let i = 0; i < input.length; i++) {
    const c = input[i];
    if (c === "{") {
      if (buf.length > 0) parts.push({ Literal: buf }), (buf = "");
      else if (parts.length && parts[parts.length - 1].Var !== undefined)
        throw "AdjacentVars";
      const end = input.indexOf("}", i + 1);
      if (end < 0) throw "UnclosedBrace";
      const name = input.substring(i + 1, end);
      if (name.length === 0) throw "EmptyVarName";
      if (seen.has(name)) throw "DuplicateVar";
      seen.add(name);
      parts.push({ Var: name });
      i = end;
    } else {
      buf += c;
    }
  }
  if (buf.length > 0) parts.push({ Literal: buf });
  return parts;
}

function extract(parts, input) {
  const dict = {};
  let cursor = 0;
  for (let idx = 0; idx < parts.length; idx++) {
    const part = parts[idx];
    if (part.Literal !== undefined) {
      if (!input.substring(cursor).startsWith(part.Literal))
        throw `LiteralMismatch(${part.Literal} @ ${cursor})`;
      cursor += part.Literal.length;
    } else {
      // Var: 找下一个 literal 作 anchor
      let end = input.length;
      for (let j = idx + 1; j < parts.length; j++) {
        if (parts[j].Literal !== undefined) {
          const off = input.substring(cursor).indexOf(parts[j].Literal);
          if (off < 0) throw `AnchorNotFound(${parts[j].Literal})`;
          end = cursor + off;
          break;
        }
      }
      dict[part.Var] = input.substring(cursor, end);
      cursor = end;
    }
  }
  if (cursor !== input.length) throw `TrailingInput(cursor=${cursor}, len=${input.length})`;
  return dict;
}

const cases = [
  {
    name: "audit: operation single segment",
    tpl: parse("/v1/audit/operation/{rest}"),
    input: "/v1/audit/operation/123",
    expect: { rest: "123" }
  },
  {
    name: "audit: operation multi segment",
    tpl: parse("/v1/audit/operation/{rest}"),
    input: "/v1/audit/operation/foo/bar/baz",
    expect: { rest: "foo/bar/baz" }
  },
  {
    name: "audit: export multi segment (compliance/operation/csv)",
    tpl: parse("/v1/audit/export/{rest}"),
    input: "/v1/audit/export/compliance/operation/csv",
    expect: { rest: "compliance/operation/csv" }
  },
  {
    name: "audit: export compliance/operation/json",
    tpl: parse("/v1/audit/export/{rest}"),
    input: "/v1/audit/export/compliance/operation/json",
    expect: { rest: "compliance/operation/json" }
  },
  {
    name: "audit: retention stage (P2 endpoint)",
    tpl: parse("/v1/audit/retention/{rest}"),
    input: "/v1/audit/retention/stage/2026-07-01",
    expect: { rest: "stage/2026-07-01" }
  },
  {
    name: "audit: login multi segment",
    tpl: parse("/v1/audit/login/{rest}"),
    input: "/v1/audit/login/sessions/abc",
    expect: { rest: "sessions/abc" }
  }
];

let ok = true;
for (const c of cases) {
  try {
    const got = extract(c.tpl, c.input);
    const match = JSON.stringify(got) === JSON.stringify(c.expect);
    if (match) {
      console.log(`✅ ${c.name}`);
      console.log(`   input : ${c.input}`);
      console.log(`   parsed: ${JSON.stringify(got)}`);
    } else {
      console.error(`✗ ${c.name}`);
      console.error(`   expected ${JSON.stringify(c.expect)}`);
      console.error(`   got      ${JSON.stringify(got)}`);
      ok = false;
    }
  } catch (e) {
    console.error(`✗ ${c.name}: ${e}`);
    ok = false;
  }
}

console.log("");
if (ok) {
  console.log("✅ audit router {rest} captures multi-segment paths");
  console.log("   /v1/audit/export/{rest} CAN forward /v1/audit/export/compliance/operation/csv");
  console.log("   /v1/audit/retention/{rest} CAN forward /v1/audit/retention/stage/<date>");
} else {
  console.log("❌ audit router {rest} does NOT handle multi-segment paths");
  console.log("   FIX: change {rest} to {rest:.*} or split routes per export/retention endpoint");
}

process.exit(ok ? 0 : 1);
