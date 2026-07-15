/**
 * Static preview server for the browser E2E suite.
 *
 * Unlike `vite preview`, this server can attach per-route COOP/COEP/CORP
 * headers so the isolated-mode tests can load SharedArrayBuffer + module
 * workers while keeping non-isolated routes available on the same origin.
 */

import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, resolve, sep } from 'node:path';

const root = process.cwd();
const distDir = join(root, 'dist');
const publicDir = join(root, 'public');
const port = Number(process.argv.find((arg) => arg.startsWith('--port='))?.slice(7) ?? '5173');

const MIME_TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.wasm': 'application/wasm',
  '.json': 'application/json',
  '.css': 'text/css',
  '.svg': 'image/svg+xml',
};

const documentHeaders = {
  '/isolated': {
    'Cross-Origin-Opener-Policy': 'same-origin',
    'Cross-Origin-Embedder-Policy': 'require-corp',
  },
  '/isolated/': {
    'Cross-Origin-Opener-Policy': 'same-origin',
    'Cross-Origin-Embedder-Policy': 'require-corp',
  },
};

function contentType(path) {
  return MIME_TYPES[extname(path)] ?? 'application/octet-stream';
}

async function tryRead(path) {
  try {
    return await readFile(path);
  } catch {
    return undefined;
  }
}

function isWithin(rootDir, targetPath) {
  const normalizedRoot = resolve(rootDir) + sep;
  const normalizedTarget = resolve(targetPath);
  return normalizedTarget === rootDir || normalizedTarget.startsWith(normalizedRoot);
}

function safePath(baseDir, pathname) {
  const decoded = decodeURIComponent(pathname);
  const target = resolve(baseDir, decoded.replace(/^\/+/, ''));
  return isWithin(baseDir, target) ? target : undefined;
}

async function tryReadFrom(baseDir, pathname) {
  const target = safePath(baseDir, pathname);
  if (!target) return undefined;
  const data = await tryRead(target);
  return data ? { path: target, data } : undefined;
}

function isWorkerPath(pathname) {
  return pathname === '/worker.js' || pathname === '/messages.js';
}

function isWasmPath(pathname) {
  return pathname.startsWith('/wasm/');
}

function setCorsHeaders(res) {
  res.setHeader('Cross-Origin-Resource-Policy', 'cross-origin');
  res.setHeader('Cross-Origin-Embedder-Policy', 'require-corp');
}

const server = createServer(async (req, res) => {
  const url = new URL(req.url ?? '/', 'http://localhost');
  const pathname = url.pathname;

  const setHeaders = (headers) => {
    for (const [key, value] of Object.entries(headers)) {
      res.setHeader(key, value);
    }
  };

  let result;
  let routeType = 'file';

  if (pathname === '/' || pathname === '/index.html' || pathname === '/isolated' || pathname === '/isolated/') {
    result = { path: resolve(distDir, 'index.html'), data: await tryRead(resolve(distDir, 'index.html')) };
    setHeaders(documentHeaders[pathname] ?? {});
    routeType = 'document';
  } else if (pathname === '/fault/wrong-mime.js') {
    // Test-only route that simulates a wasm-bindgen JS file served with the
    // wrong MIME type so the module import fails in the worker.
    setCorsHeaders(res);
    res.setHeader('Content-Type', 'text/plain; charset=utf-8');
    res.end('this is not a javascript module');
    return;
  } else if (isWorkerPath(pathname) || isWasmPath(pathname)) {
    // Workers and WASM loaded in a COEP context need explicit COEP + CORP.
    result = await tryReadFrom(publicDir, pathname);
    setCorsHeaders(res);
  } else if (pathname.startsWith('/assets/')) {
    result = await tryReadFrom(distDir, pathname);
    setHeaders({ 'Cross-Origin-Resource-Policy': 'cross-origin' });
  } else {
    // Public files (e.g. favicon) fall back to dist.
    result = (await tryReadFrom(publicDir, pathname)) ?? (await tryReadFrom(distDir, pathname));
  }

  if (!result) {
    res.statusCode = routeType === 'document' ? 404 : 403;
    res.end(routeType === 'document' ? 'not found' : 'forbidden');
    return;
  }

  if (!result.data) {
    res.statusCode = 404;
    res.end('not found');
    return;
  }

  const ext = extname(result.path);
  if (ext === '.js' || ext === '.mjs' || ext === '.wasm') {
    setHeaders({ 'Cross-Origin-Resource-Policy': 'cross-origin' });
  }

  res.setHeader('Content-Type', contentType(result.path));
  res.end(result.data);
});

server.listen(port, () => {
  console.log(`Preview server listening on http://localhost:${port}`);
});
