#!/usr/bin/env node
// Gate: unsafe code is contained and every block is justified.
//
// agent-universe ships a single crate (gsn-core). The mechanical boundary that
// matters here is that every `unsafe` block/fn/impl carries a `// SAFETY:`
// comment naming the invariant that makes it sound. The moment someone adds an
// undocumented unsafe block, this fails.
//
// Usage:
//   node scripts/check-unsafe-containment.mjs [dir]   fail on any violation (default gsn-core/src)
//   node scripts/check-unsafe-containment.mjs --list  report only, exit 0

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

function stripNonCode(src) {
  // keep only code by blanking comments and string/char literals; we need to
  // detect the `unsafe` keyword and the SAFETY comment separately, so here we
  // only blank string literals (to avoid false positives in quoted text).
  let out = '';
  let i = 0;
  const n = src.length;
  while (i < n) {
    const c = src[i];
    const m = /^(?:b)?r(#*)"/.exec(src.slice(i));
    if (m) {
      const hashes = m[1].length;
      const opener = i + m[0].length;
      const close = '"' + '#'.repeat(hashes);
      let j = opener;
      while (j < n && src.slice(j, j + close.length) !== close) j++;
      j = Math.min(n, j + close.length);
      out += src.slice(i, j).replace(/[^\n]/g, ' ');
      i = j;
      continue;
    }
    if (c === '"' || (c === 'b' && src[i + 1] === '"')) {
      let j = i + (c === 'b' ? 2 : 1);
      while (j < n && src[j] !== '"') {
        if (src[j] === '\\') j++;
        j++;
      }
      j = Math.min(n, j + 1);
      out += src.slice(i, j).replace(/[^\n]/g, ' ');
      i = j;
      continue;
    }
    out += c;
    i++;
  }
  return out;
}

const UNSAFE = /unsafe\s*(\{|fn\b|impl\b)/g;
const SAFETY = /\/\/\s*SAFETY:/g;

const files = walk(SCAN, []);
const problems = [];
const present = [];
for (const f of files) {
  const raw = fs.readFileSync(f, 'utf8');
  const code = stripNonCode(raw);
  const blocks = (code.match(UNSAFE) || []).length;
  if (blocks === 0) continue;
  const safety = (raw.match(SAFETY) || []).length;
  const rel = path.relative(REPO, f).replace(/\\/g, '/');
  present.push({ rel, blocks, safety });
  if (safety < blocks) {
    problems.push(`${rel}: ${blocks} unsafe item(s) but only ${safety} "// SAFETY:" comment(s)`);
  }
}

console.log('unsafe code is present in exactly these files:');
if (present.length === 0) {
  console.log('  (none)');
} else {
  for (const p of present) {
    console.log(`  ${p.rel}  (${p.blocks} unsafe item(s), ${p.safety} SAFETY comment(s))`);
  }
}

if (problems.length === 0) {
  console.log('\nOK: every unsafe item carries a written SAFETY justification.');
  process.exit(0);
}
console.log('\n' + problems.length + ' violation(s):');
for (const p of problems) console.log('  ' + p);
console.log(
  '\nAdd a // SAFETY: comment on the line directly above the unsafe item, stating the\n' +
    'invariant the caller/type upholds that makes the block sound.',
);
process.exit(listOnly ? 0 : 1);
