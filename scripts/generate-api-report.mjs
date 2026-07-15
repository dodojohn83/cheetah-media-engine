#!/usr/bin/env node
/**
 * Generate a public API report and event/error table for the Web v1 handoff.
 *
 * Output: docs/web-v1-handoff/api-report.md (or the directory passed as argv[2]).
 */

import { execSync } from 'node:child_process';
import { readdirSync, readFileSync, statSync, writeFileSync, existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const outDir = process.argv[2] ? resolve(process.argv[2]) : resolve(root, 'docs/web-v1-handoff');
const outFile = join(outDir, 'api-report.md');

function read(p) {
  return readFileSync(p, 'utf8');
}

function linesFor(text, startLine, endLine) {
  return text.split(/\r?\n/).slice(startLine - 1, endLine).join('\n');
}

function* rustTopLevel(text) {
  const ls = text.split(/\r?\n/);
  let i = 0;
  while (i < ls.length) {
    const line = ls[i];
    const m = line.match(/^(\s*)pub(\s*\([^)]*\))?\s+(mod|use|struct|enum|fn|type|trait|const)\s+/);
    if (m) {
      let buf = line.trim();
      i += 1;
      while (i < ls.length && !buf.endsWith(';') && !buf.endsWith('{')) {
        buf += ' ' + ls[i].trim();
        i += 1;
      }
      yield buf.replace(/\s+/g, ' ');
    } else {
      i += 1;
    }
  }
}

function rustPublicApi(crateDir) {
  const lib = join(crateDir, 'src', 'lib.rs');
  if (!existsSync(lib)) return [];
  const text = read(lib);
  const items = [];
  for (const item of rustTopLevel(text)) {
    if (item.includes('(crate)') || item.includes('(super)')) continue;
    items.push(item);
  }
  return items;
}

function crateName(crateDir) {
  const cargo = read(join(crateDir, 'Cargo.toml'));
  const m = cargo.match(/^name\s*=\s*"([^"]+)"/m);
  return m ? m[1] : dirname(crateDir);
}

function tsExports(indexPath) {
  const text = read(indexPath);
  const out = [];
  // export { a, b } from './module' (and export type { ... } from ...)
  for (const block of text.matchAll(/^export\s+(?:type\s+)?\{\s*([^}]+)\}\s*from\s+['"]([^'"]+)['"]\s*;?$/gm)) {
    const names = block[1].split(/,\s*/).map((s) => s.trim()).filter(Boolean);
    out.push(`from ${block[2]}: ${names.join(', ')}`);
  }
  // export { a, b }; or export type { a, b };
  for (const block of text.matchAll(/^export\s+(?:type\s+)?\{\s*([^}]+)\}\s*;?$/gm)) {
    const names = block[1].split(/,\s*/).map((s) => s.trim()).filter(Boolean);
    out.push(`{ ${names.join(', ')} }`);
  }
  // export * from './module'
  for (const m of text.matchAll(/^export\s+\*\s+from\s+['"]([^'"]+)['"]\s*;?$/gm)) {
    out.push(`* from ${m[1]}`);
  }
  // single-line/export declaration first line: export const/type/interface/class/function X ... ; or {
  for (const m of text.matchAll(/^export\s+((?:type|interface|class|function|const)\s+[^;{]+)[;{]/gm)) {
    out.push(`export ${m[1].replace(/\s+/g, ' ').trim()}`);
  }
  return out;
}

function extractMultilineExport(text, kind, name) {
  const re = new RegExp(`export\\s+(?:type|interface|class)\\s+${name}\\s*=?\\s*`);
  const start = text.search(re);
  if (start === -1) return undefined;
  const rest = text.slice(start);
  // Find the matching statement end: semicolon for type alias, opening brace for interface/class.
  let depth = 0;
  let inString = false;
  let stringChar = '';
  let end = 0;
  let started = false;
  for (let i = 0; i < rest.length; i++) {
    const c = rest[i];
    if (inString) {
      if (c === '\\') { i++; continue; }
      if (c === stringChar) inString = false;
      continue;
    }
    if (c === '"' || c === "'" || c === '`') { inString = true; stringChar = c; continue; }
    if (c === '{') { started = true; depth++; continue; }
    if (c === '}') { depth--; if (started && depth === 0) { end = i + 1; break; } continue; }
    if (c === ';' && !started) { end = i + 1; break; }
  }
  return rest.slice(0, end).replace(/\s+/g, ' ').trim();
}

function extractCodeUnion(text, name) {
  const snippet = extractMultilineExport(text, 'type', name);
  if (!snippet) return undefined;
  const literals = [...snippet.matchAll(/['"]([^'"]+)['"]/g)].map((m) => m[1]);
  return literals;
}

function* walkDir(dir, ext) {
  if (!existsSync(dir)) return;
  for (const entry of readdirSync(dir)) {
    const p = join(dir, entry);
    const st = statSync(p);
    if (st.isDirectory()) {
      yield* walkDir(p, ext);
    } else if (entry.endsWith(ext)) {
      yield p;
    }
  }
}

function eventErrorEntries() {
  const entries = [];
  const seen = new Set();
  for (const dir of ['packages/runtime/src', 'packages/web/src']) {
    const base = join(root, dir);
    for (const file of walkDir(base, '.ts')) {
      const text = read(file);
      const rel = file.slice(root.length + 1);
      // export class ...Error / export type ...EventType / export interface ...Event
      for (const m of text.matchAll(/export\s+(?:type|interface|class)\s+([A-Za-z_$][A-Za-z0-9_$]*(?:Error|Event|EventType|Code|Payload))(?![A-Za-z0-9_$])/g)) {
        const name = m[1];
        const kind = m[0].includes('class') ? 'class' : m[0].includes('interface') ? 'interface' : 'type';
        const key = `${rel}:${name}`;
        if (seen.has(key)) continue;
        seen.add(key);
        let details = '';
        if (name.endsWith('Code')) {
          const values = extractCodeUnion(text, name);
          if (values && values.length > 0) details = values.map((v) => `\`${v}\``).join(', ');
        } else if (name.endsWith('EventType')) {
          const values = extractCodeUnion(text, name);
          if (values && values.length > 0) details = values.map((v) => `\`${v}\``).join(', ');
        }
        const snippet = extractMultilineExport(text, kind, name);
        entries.push({ name, kind, file: rel, details, snippet });
      }
    }
  }
  return entries;
}

function gitCommit() {
  try {
    return execSync('git rev-parse --short HEAD', { cwd: root, encoding: 'utf8' }).trim();
  } catch {
    return 'unknown';
  }
}

// Build report
let md = `# Cheetah Media Engine Web v1 API Report\n\n`;
md += `Generated: ${new Date().toISOString()}\n`;
md += `Commit: ${gitCommit()}\n\n`;

md += `## Rust Crate Public API\n\n`;
md += `Top-level public declarations from each crate's \`src/lib.rs\`.\n\n`;
for (const entry of readdirSync(join(root, 'crates'))) {
  const crateDir = join(root, 'crates', entry);
  if (!statSync(crateDir).isDirectory()) continue;
  if (!existsSync(join(crateDir, 'Cargo.toml'))) continue;
  const name = crateName(crateDir);
  const items = rustPublicApi(crateDir);
  if (items.length === 0) continue;
  md += `### ${name}\n\n`;
  for (const item of items) {
    md += `- \`${item.replace(/`/g, '\\`')}\`\n`;
  }
  md += '\n';
}

md += `## TypeScript Public API\n\n`;
md += `Exports from \`packages/*/src/index.ts\`.\n\n`;
for (const pkg of ['runtime', 'web', 'components']) {
  const idx = join(root, 'packages', pkg, 'src', 'index.ts');
  if (!existsSync(idx)) continue;
  const exports = tsExports(idx);
  md += `### @cheetah-media/${pkg}\n\n`;
  for (const exp of exports) {
    md += `- \`${exp.replace(/`/g, '\\`')}\`\n`;
  }
  md += '\n';
}

md += `## Events, Errors and Message Payloads\n\n`;
md += `| Name | Kind | Source | Values / Note |\n`;
md += `|------|------|--------|---------------|\n`;
for (const e of eventErrorEntries()) {
  const note = e.details || (e.snippet ? '`' + e.snippet.slice(0, 120).replace(/`/g, '\\`') + (e.snippet.length > 120 ? '...' : '') + '`' : '');
  md += `| ${e.name} | ${e.kind} | ${e.file} | ${note} |\n`;
}
md += '\n';

md += `## Notes\n\n`;
md += `- This report is a snapshot of exported identifiers; see the source files for full signatures and documentation.\n`;
md += `- Event/error values are extracted from string-literal unions where possible; dynamic objects and class bodies are summarized.\n`;

if (!existsSync(outDir)) {
  throw new Error(`Output directory does not exist: ${outDir}`);
}
writeFileSync(outFile, md);
console.log(`[api-report] wrote ${outFile}`);
