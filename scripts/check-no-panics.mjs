#!/usr/bin/env node
// Gate: no panic paths in production code.
//
// A bare `grep unwrap` over-reports because module docs quote the upstream
// `.unwrap()` calls they describe, and it under-reports the intent because test
// code legitimately uses them. So this scanner:
//   1. strips line/block comments and string/char literals (equal-length so line
//      numbers are preserved),
//   2. removes `#[cfg(test)]` blocks by brace matching,
//   3. then matches panic patterns: .unwrap() / .expect( / panic! / unreachable! /
//      todo! / unimplemented!
//
// assert! is intentionally NOT matched: argument validation in constructors is
// treated as a programmer error, not a runtime panic path.
//
// Usage:
//   node scripts/check-no-panics.mjs [dir]   fail on any hit (default: gsn-core/src)
//   node scripts/check-no-panics.mjs --list   report only, exit 0

import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const REPO = path.resolve(
  path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1')),
  '..',
);
const listOnly = process.argv.includes('--list');
const arg = process.argv.slice(2).find((a) => !a.startsWith('--'));
const SCAN = path.resolve(REPO, arg || 'gsn-core/src');

function walk(dir, out) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    if (e.name === 'target' || e.name === 'node_modules' || e.name === '.git') continue;
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p, out);
    else if (e.name.endsWith('.rs')) out.push(p);
  }
  return out;
}

// Replace comments/strings/chars with equal-length spaces so line numbers stay
// valid. The source is scanned char by char.
function stripLiterals(src) {
  let out = '';
  let i = 0;
  const n = src.length;
  const blank = (s) => s.replace(/[^\n]/g, ' ');
  while (i < n) {
    const c = src[i];
    // line comment
    if (c === '/' && src[i + 1] === '/') {
      let j = i;
      while (j < n && src[j] !== '\n') j++;
      out += blank(src.slice(i, j));
      i = j;
      continue;
    }
    // block comment
    if (c === '/' && src[i + 1] === '*') {
      let j = i + 2;
      while (j < n && !(src[j] === '*' && src[j + 1] === '/')) j++;
      j = Math.min(n, j + 2);
      out += blank(src.slice(i, j));
      i = j;
      continue;
    }
    // 字符串字面量：raw（r#"..."# / br#"..."#）、byte（b"..."）、普通（"..."）。
    // 每一种都必须先用前缀正则/前瞻严格确认，避免把函数名里恰好出现的
    // `br`/`b`/`r`（如 broadcast）误判为字符串开头而吃到后面的大括号。
    // 1) raw string —— 前缀必须是 r(#*)" 或 br(#*)"。
    const rawM = /^(?:b)?r(#*)"/.exec(src.slice(i));
    if (rawM) {
      const hashes = rawM[1].length;
      const opener = i + rawM[0].length;
      const close = '"' + '#'.repeat(hashes);
      let j = opener;
      while (j < n && src.slice(j, j + close.length) !== close) j++;
      j = Math.min(n, j + close.length);
      out += blank(src.slice(i, j));
      i = j;
      continue;
    }
    // 2) byte string —— b 后必须紧跟引号。
    if (c === 'b' && src[i + 1] === '"') {
      let j = i + 2;
      while (j < n && src[j] !== '"') {
        if (src[j] === '\\') j++;
        j++;
      }
      j = Math.min(n, j + 1);
      out += blank(src.slice(i, j));
      i = j;
      continue;
    }
    // 3) 普通字符串。
    if (c === '"') {
      let j = i + 1;
      while (j < n && src[j] !== '"') {
        if (src[j] === '\\') j++;
        j++;
      }
      j = Math.min(n, j + 1);
      out += blank(src.slice(i, j));
      i = j;
      continue;
    }
    // char literal 必须与 lifetime 区分——否则 `Formatter<'_>` 里的 `'` 会被
    // 当成 char literal 开头，一直吞到文件末尾的下一个 `'`。
    //   · 转义 char：' 后紧跟 `\`，在同一行内按「转义对」找闭合 '（含 '\''）；
    //   · 单字符 char：'X'（' 后一个非 ' 非 \ 非换行字符，再紧跟 '）；
    //   · 其余（'_ / 'a / 'static）是 lifetime，' 当普通字符保留，不吞后续。
    if (c === "'") {
      const after = src[i + 1];
      let j = -1;
      if (after === '\\') {
        let k = i + 1;
        while (k < n && src[k] !== '\n') {
          if (src[k] === '\\') {
            k += 2;
            continue;
          }
          if (src[k] === "'") break;
          k++;
        }
        if (k < n && src[k] === "'") j = k + 1;
      } else if (
        after !== undefined &&
        after !== "'" &&
        after !== '\n' &&
        src[i + 2] === "'"
      ) {
        j = i + 3;
      }
      if (j >= 0) {
        out += blank(src.slice(i, j));
        i = j;
        continue;
      }
      // lifetime label：保留 ' 为普通字符，不吞后续内容。
      out += c;
      i++;
      continue;
    }
    // 普通字符：原样保留并推进。
    out += c;
    i++;
  }
  return out;
}

// Remove `#[cfg(test)]` attribute blocks (and the item they annotate) by brace
// matching. Equal-length replacement keeps line numbers.
function removeCfgTest(src) {
  let out = src;
  const re = /#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]/g;
  let m;
  const spans = [];
  while ((m = re.exec(src))) spans.push(m.index);
  for (let start of spans) {
    // find the first '{' after the attribute, then match braces
    let i = out.indexOf('{', start);
    if (i < 0) continue;
    let depth = 0;
    let j = i;
    for (; j < out.length; j++) {
      if (out[j] === '{') depth++;
      else if (out[j] === '}') {
        depth--;
        if (depth === 0) { j++; break; }
      }
    }
    out = out.slice(0, start) + out.slice(start, j).replace(/[^\n]/g, ' ') + out.slice(j);
  }
  return out;
}

const PATTERNS = [
  { name: 'unwrap()', re: /\.unwrap\s*\(\s*\)/g },
  { name: 'expect()', re: /\.expect\s*\(/g },
  { name: 'panic!', re: /\bpanic!\s*\(/g },
  { name: 'unreachable!', re: /\bunreachable!\s*\(/g },
  { name: 'todo!', re: /\btodo!\s*\(/g },
  { name: 'unimplemented!', re: /\bunimplemented!\s*\(/g },
];

const files = walk(SCAN, []);
let total = 0;
for (const f of files) {
  const raw = fs.readFileSync(f, 'utf8');
  const code = removeCfgTest(stripLiterals(raw));
  const lines = code.split('\n');
  for (const p of PATTERNS) {
    lines.forEach((line, idx) => {
      p.re.lastIndex = 0;
      if (p.re.test(line)) {
        total++;
        console.log(
          `${path.relative(REPO, f).replace(/\\/g, '/')}:${idx + 1}: ${p.name}  ${line.trim()}`,
        );
      }
    });
  }
}

console.log(`\nTOTAL production panic sites: ${total}`);
if (total === 0) {
  console.log('OK: no panic paths in production code.');
  process.exit(0);
}
process.exit(listOnly ? 0 : 1);
