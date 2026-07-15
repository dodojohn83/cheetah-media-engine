/**
 * Build the Rust/WASM engine and copy the worker + wasm-bindgen artifacts
 * into the demo's public directory so the preview server can serve them.
 */

import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { existsSync } from 'node:fs';
import { cp, mkdir, rm } from 'node:fs/promises';
import { execSync } from 'node:child_process';

const root = join(fileURLToPath(import.meta.url), '..', '..', '..', '..');
const publicDir = join(root, 'apps', 'web-demo', 'public');
const wasmDir = join(publicDir, 'wasm');
const workerSrc = join(root, 'packages', 'runtime', 'dist', 'worker.js');
const workerDst = join(publicDir, 'worker.js');

const profile = process.env.WASM_PROFILE || 'release';
const targetDir = join(root, 'target', 'wasm32-unknown-unknown', profile);
const wasmFile = join(targetDir, 'cheetah_media_web_bindings.wasm');

if (!existsSync(wasmFile)) {
  const args = ['build', '-p', 'cheetah-media-web-bindings', '--target', 'wasm32-unknown-unknown'];
  if (profile === 'release') {
    args.push('--release');
  }
  execSync(`cargo ${args.join(' ')}`, { stdio: 'inherit', cwd: root });
}

const tmpDir = join(root, 'target', 'wasm-pkg');
await rm(tmpDir, { recursive: true, force: true });
await mkdir(tmpDir, { recursive: true });

execSync(
  `wasm-bindgen --target web --out-dir ${tmpDir} ${wasmFile}`,
  { stdio: 'inherit', cwd: root },
);

await mkdir(wasmDir, { recursive: true });
for (const file of ['cheetah_media_web_bindings.js', 'cheetah_media_web_bindings_bg.wasm']) {
  await cp(join(tmpDir, file), join(wasmDir, file));
}

if (!existsSync(workerSrc)) {
  execSync('pnpm --filter @cheetah-media/runtime build', { stdio: 'inherit', cwd: root });
}
await cp(workerSrc, workerDst);

console.log('[build-wasm] copied worker and wasm to', publicDir);
