/**
 * Browser capability report used to decide isolation level and codec strategy.
 */
export interface CapabilityReport {
  secureContext: boolean;
  crossOriginIsolated: boolean;
  sharedArrayBuffer: boolean;
  atomics: boolean;
  simd: boolean;
  threads: boolean;
  webCodecs: boolean;
  webAudio: boolean;
  offscreenCanvas: boolean;
}

function hasGlobal(name: string): boolean {
  return typeof globalThis !== 'undefined' && name in globalThis;
}

function hasSimdSupport(): boolean {
  if (!hasGlobal('WebAssembly')) return false;
  // Minimal SIMD module: v128.any_true (0xfd 0x0d).
  const bytes = new Uint8Array([
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
    0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f,
    0x03, 0x02, 0x01, 0x00,
    0x0a, 0x0b, 0x01, 0x09, 0x00, 0x00, 0xfd, 0x0d, 0x00, 0x00, 0x0b,
  ]);
  return (WebAssembly as unknown as { validate?: (buffer: ArrayBuffer) => boolean }).validate?.(bytes.buffer) ?? false;
}

function canUseThreads(): boolean {
  if (!hasGlobal('Worker')) return false;
  if (!hasGlobal('SharedArrayBuffer')) return false;
  if (!hasGlobal('Atomics')) return false;
  try {
    // Shared memory requires cross-origin isolated in modern browsers.
    new SharedArrayBuffer(4);
    return true;
  } catch {
    return false;
  }
}

/**
 * Detect runtime capabilities. Safe to call in node/vitest because it only
 * probes globals and catches failures.
 */
export function detectCapabilities(): CapabilityReport {
  return {
    secureContext: (globalThis as unknown as { isSecureContext?: boolean }).isSecureContext ?? false,
    crossOriginIsolated: (globalThis as unknown as { crossOriginIsolated?: boolean }).crossOriginIsolated ?? false,
    sharedArrayBuffer: hasGlobal('SharedArrayBuffer'),
    atomics: hasGlobal('Atomics'),
    simd: hasSimdSupport(),
    threads: canUseThreads(),
    webCodecs: hasGlobal('VideoDecoder') && hasGlobal('AudioDecoder'),
    webAudio: hasGlobal('AudioContext') || hasGlobal('webkitAudioContext'),
    offscreenCanvas: hasGlobal('OffscreenCanvas'),
  };
}
