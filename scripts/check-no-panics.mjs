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
    // string: b"..." or "...", also raw strings r#"..."# / br#"..."#
    if (c === '"' || (c === 'b' && src[i + 1] === '"') || (c === 'r' && src[i + 1] === '"') ||
        (c === 'r' && src[i + 1] === '#') || (c === 'b' && src[i + 1] === 'r')) {
      // raw string
      const m = /^(?:b)?r(#*)"/.exec(src.slice(i));
      if (m) {
        const hashes = m[1].length;
        const opener = i + m[0].length;
        const close = '"' + '#'.repeat(hashes);
        let j = opener;
        while (j < n && src.slice(j, j + close.length) !== close) j++;
        j = Math.min(n, j + close.length);
        out += blank(src.slice(i, j));
        i = j;
        continue;
      }
      if (c === '"' || c === 'b') {
        let j = i + (c === 'b' ? 2 : 1);
        while (j < n && src[j] !== '"') {
          if (src[j] === '\\') j++;
          j++;
        }
        j = Math.min(n, j + 1);
        out += blank(src.slice(i, j));
        i = j;
        continue;
      }
    }
    // char literal
    if (c === "'") {
      let j = i + 1;
      while (j < n && src[j] !== "'") {
        if (src[j] === '\\') j++;
        j++;
      }
      j = Math.min(n, j + 1);
      out += blank(src.slice(i, j));
      i = j;
      continue;
    }
    // lifetime label like 'a — leave untouched (single quote followed by ident
    // then a non-char context); only treat as literal when closed.
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
