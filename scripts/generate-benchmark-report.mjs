#!/usr/bin/env node
/**
 * Summarize Criterion benchmark results into a markdown report.
 *
 * Reads target/criterion/<bench>/new/estimates.json and sample.json.
 * Output: docs/web-v1-handoff/benchmark-report.md (or <dir>/benchmark-report.md).
 */

import { execSync } from 'node:child_process';
import { readdirSync, readFileSync, writeFileSync, existsSync, statSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const outDir = process.argv[2] ? resolve(process.argv[2]) : resolve(root, 'docs/web-v1-handoff');
const outFile = join(outDir, 'benchmark-report.md');
const criterionDir = join(root, 'target', 'criterion');

function readJson(p) {
  if (!existsSync(p)) return undefined;
  return JSON.parse(readFileSync(p, 'utf8'));
}

function formatNs(ns) {
  if (ns >= 1e6) return `${(ns / 1e6).toFixed(3)} ms`;
  if (ns >= 1e3) return `${(ns / 1e3).toFixed(3)} us`;
  return `${ns.toFixed(3)} ns`;
}

function collectBenchmarks() {
  if (!existsSync(criterionDir)) return [];
  const out = [];
  for (const name of readdirSync(criterionDir)) {
    const newDir = join(criterionDir, name, 'new');
    if (!existsSync(newDir) || !statSync(newDir).isDirectory()) continue;
    const estimates = readJson(join(newDir, 'estimates.json'));
    const sample = readJson(join(newDir, 'sample.json'));
    if (!estimates) continue;
    const mean = estimates.mean?.point_estimate ?? NaN;
    const meanLower = estimates.mean?.confidence_interval?.lower_bound ?? NaN;
    const meanUpper = estimates.mean?.confidence_interval?.upper_bound ?? NaN;
    const iters = Array.isArray(sample?.iters) ? sample.iters.length : 0;
    out.push({
      name,
      mean,
      meanLower,
      meanUpper,
      stdDev: estimates.std_dev?.point_estimate ?? NaN,
      iters,
    });
  }
  return out.sort((a, b) => a.name.localeCompare(b.name));
}

function gitCommit() {
  try {
    return execSync('git rev-parse --short HEAD', { cwd: root, encoding: 'utf8' }).trim();
  } catch {
    return 'unknown';
  }
}

const benchmarks = collectBenchmarks();

let md = `# Cheetah Media Engine Web v1 Benchmark Report\n\n`;
md += `Generated: ${new Date().toISOString()}\n`;
md += `Commit: ${gitCommit()}\n`;
md += `Source: \`cargo bench -p cheetah-media-types --features std\`\n\n`;

if (benchmarks.length === 0) {
  md += 'No Criterion results found in `target/criterion`. Run `cargo bench` first.\n';
} else {
  md += `| Benchmark | Mean | 95% CI | Std Dev | Samples |\n`;
  md += `|-------------|------|--------|---------|----------|\n`;
  for (const b of benchmarks) {
    const ci = Number.isFinite(b.meanLower) && Number.isFinite(b.meanUpper)
      ? `${formatNs(b.meanLower)}–${formatNs(b.meanUpper)}`
      : 'n/a';
    md += `| ${b.name} | ${formatNs(b.mean)} | ${ci} | ${formatNs(b.stdDev)} | ${b.iters} |\n`;
  }
  md += '\n';
  md += `## Notes\n\n`;
  md += `- Times are per-iteration and are produced by Criterion.rs on the local VM.\n`;
  md += `- These are baseline measurements; target hardware will differ.\n`;
  md += `- Raw sample data is in \`target/criterion/<benchmark>/new/sample.json\`.\n`;
}

writeFileSync(outFile, md);
console.log(`[benchmark-report] wrote ${outFile}`);
