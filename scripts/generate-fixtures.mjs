#!/usr/bin/env node
/**
 * Generate real encoded media fixtures for the Web v1 acceptance matrix.
 *
 * Requirements: ffmpeg and ffprobe in PATH.
 * Output: testing/fixtures/media/ and an updated testing/fixtures/manifest.json.
 */

import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { mkdir, readFile, readdir, writeFile, cp, rm } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const root = resolve(dirname(__filename), '..');
const mediaDir = join(root, 'testing', 'fixtures', 'media');
const manifestPath = join(root, 'testing', 'fixtures', 'manifest.json');

function run(cmd, args, opts = {}) {
  try {
    return execFileSync(cmd, args, { stdio: opts.quiet ? 'pipe' : 'inherit', encoding: 'utf8', ...opts });
  } catch (err) {
    throw new Error(`${cmd} ${args.join(' ')} failed: ${err.message}`);
  }
}

function ensureTool(name) {
  try {
    execFileSync('which', [name], { stdio: 'pipe' });
  } catch {
    throw new Error(`${name} not found in PATH; install ffmpeg to generate fixtures`);
  }
}

async function sha256(filePath) {
  const data = await readFile(filePath);
  return createHash('sha256').update(data).digest('hex');
}

function ffprobeJson(filePath) {
  const out = run('ffprobe', ['-v', 'error', '-print_format', 'json', '-show_format', '-show_streams', filePath], { quiet: true });
  return JSON.parse(out);
}

async function fileSize(filePath) {
  return (await readFile(filePath)).length;
}

function extractCodec(stream) {
  if (!stream) return 'unknown';
  const name = stream.codec_name;
  if (name === 'h264') return 'h264';
  if (name === 'hevc' || name === 'h265') return 'h265';
  if (name === 'aac') return 'aac';
  if (name === 'mp3') return 'mp3';
  if (name === 'pcm_alaw') return 'g711a';
  if (name === 'pcm_mulaw') return 'g711u';
  return name;
}

function streamMeta(probe) {
  const video = probe.streams.find((s) => s.codec_type === 'video');
  const audio = probe.streams.find((s) => s.codec_type === 'audio');
  const stream = video || audio;
  return {
    codec: extractCodec(stream),
    resolution: video ? `${video.width}x${video.height}` : undefined,
    frame_rate: video ? Math.round(video.r_frame_rate.split('/').reduce((a, b) => a / b)) : undefined,
    sample_rate: audio ? Number(audio.sample_rate) : undefined,
    channels: audio ? Number(audio.channels) : undefined,
    duration_ms: Math.round((parseFloat(probe.format?.duration) || 0) * 1000),
  };
}

async function addFixtureEntry(entries, id, filePath, description, protocol, extra = {}) {
  const relative = filePath.replace(mediaDir + '/', '').replace(/\\/g, '/');
  const probe = ffprobeJson(filePath);
  const meta = streamMeta(probe);

  const existing = entries.find((e) => e.id === id);
  const entry = {
    id,
    description,
    source: {
      type: 'download',
      generator: `ffmpeg ${run('ffmpeg', ['-version'], { quiet: true }).split('\n')[0].split(' ')[2]}`,
      url: `media/${relative}`,
      commit: undefined,
    },
    license: 'MIT-0',
    hash: await sha256(filePath),
    protocol,
    codec: extra.codec ?? meta.codec,
    resolution: extra.resolution ?? meta.resolution,
    frame_rate: extra.frame_rate ?? meta.frame_rate,
    sample_rate: extra.sample_rate ?? meta.sample_rate,
    channels: extra.channels ?? meta.channels,
    duration_ms: extra.duration_ms ?? meta.duration_ms,
    anomaly: extra.anomaly,
    expected: extra.expected ?? `real ${protocol} ${meta.codec} playback evidence`,
  };
  if (existing) {
    Object.assign(existing, entry);
  } else {
    entries.push(entry);
  }
}

async function generateFmp4Clip(outFile, { size = '640x480', videoCodec = 'libx264', tag, duration = '2', audioRate = '48000' }) {
  const videoArgs = videoCodec === 'libx265'
    ? ['-c:v', 'libx265', '-preset', 'ultrafast', '-b:v', '600k', '-g', '30', '-tag:v', 'hvc1']
    : ['-c:v', 'libx264', '-preset', 'ultrafast', '-b:v', '600k', '-g', '30'];
  await mkdir(dirname(outFile), { recursive: true });
  run('ffmpeg', [
    '-f', 'lavfi', '-i', `testsrc=size=${size}:rate=30`,
    '-f', 'lavfi', '-i', `sine=frequency=1000:sample_rate=${audioRate}`,
    '-pix_fmt', 'yuv420p',
    ...videoArgs,
    '-c:a', 'aac', '-b:a', '128k',
    '-movflags', 'frag_keyframe+empty_moov+default_base_moof',
    '-t', duration,
    outFile, '-y',
  ]);
}

async function main() {
  ensureTool('ffmpeg');
  ensureTool('ffprobe');
  await mkdir(mediaDir, { recursive: true });

  const rawManifest = await readFile(manifestPath, 'utf8').catch(() => '{"schema_version":"1.0","fixtures":[]}');
  const manifest = JSON.parse(rawManifest);
  // Keep only synthetic fixtures; all real/download fixtures are regenerated below.
  const entries = (manifest.fixtures || []).filter((e) => e.source?.type === 'synthetic');

  // --- Single fMP4 clips ---
  await generateFmp4Clip(join(mediaDir, 'h264-fmp4', 'clip.mp4'), { size: '1280x720', videoCodec: 'libx264' });
  await generateFmp4Clip(join(mediaDir, 'h265-fmp4', 'clip.mp4'), { size: '640x480', videoCodec: 'libx265' });
  await generateFmp4Clip(join(mediaDir, 'h264-http-fmp4', 'clip.mp4'), { size: '640x480', videoCodec: 'libx264' });
  await generateFmp4Clip(join(mediaDir, 'h264-ws-fmp4', 'clip.mp4'), { size: '640x480', videoCodec: 'libx264' });

  await mkdir(join(mediaDir, 'mp3-fmp4'), { recursive: true });
  run('ffmpeg', [
    '-f', 'lavfi', '-i', 'sine=frequency=1000:sample_rate=44100',
    '-c:a', 'mp3', '-b:a', '128k',
    '-movflags', 'frag_keyframe+empty_moov+default_base_moof',
    '-t', '2',
    join(mediaDir, 'mp3-fmp4', 'clip.mp4'), '-y',
  ]);

  await mkdir(join(mediaDir, 'aac-fmp4'), { recursive: true });
  run('ffmpeg', [
    '-f', 'lavfi', '-i', 'sine=frequency=1000:sample_rate=48000',
    '-c:a', 'aac', '-b:a', '128k',
    '-movflags', 'frag_keyframe+empty_moov+default_base_moof',
    '-t', '2',
    join(mediaDir, 'aac-fmp4', 'audio.mp4'), '-y',
  ]);

  // --- HLS with fMP4 segments ---
  const hlsFmp4Dir = join(mediaDir, 'hls-h264-fmp4');
  await mkdir(hlsFmp4Dir, { recursive: true });
  run('ffmpeg', [
    '-f', 'lavfi', '-i', 'testsrc=size=640x480:rate=30',
    '-f', 'lavfi', '-i', 'sine=frequency=1000:sample_rate=48000',
    '-pix_fmt', 'yuv420p', '-c:v', 'libx264', '-preset', 'ultrafast', '-b:v', '600k', '-g', '30',
    '-c:a', 'aac', '-b:a', '128k',
    '-t', '2',
    '-f', 'hls', '-hls_time', '1', '-hls_list_size', '0',
    '-hls_segment_type', 'fmp4',
    '-hls_flags', 'independent_segments',
    '-master_pl_name', 'master.m3u8',
    join(hlsFmp4Dir, 'playlist.m3u8'), '-y',
  ]);

  // --- FLV (shared for HTTP-FLV and WS-FLV) ---
  await mkdir(join(mediaDir, 'h264-flv'), { recursive: true });
  run('ffmpeg', [
    '-f', 'lavfi', '-i', 'testsrc=size=640x480:rate=30',
    '-f', 'lavfi', '-i', 'sine=frequency=1000:sample_rate=44100',
    '-pix_fmt', 'yuv420p', '-c:v', 'libx264', '-preset', 'ultrafast', '-b:v', '600k', '-g', '30',
    '-c:a', 'aac', '-b:a', '128k',
    '-t', '2',
    join(mediaDir, 'h264-flv', 'clip.flv'), '-y',
  ]);

  // --- G.711 in fMP4 (MOV/MP4 with mp42 brand) ---
  for (const [dir, codec] of [['g711a', 'pcm_alaw'], ['g711u', 'pcm_mulaw']]) {
    await mkdir(join(mediaDir, dir), { recursive: true });
    // Clean up previous wav/raw attempts.
    for (const old of ['clip.wav', 'clip.mp3']) {
      try {
        await rm(join(mediaDir, dir, old), { force: true });
      } catch {
        // ignore
      }
    }
    run('ffmpeg', [
      '-f', 'lavfi', '-i', 'sine=frequency=1000:sample_rate=8000',
      '-c:a', codec,
      '-t', '1',
      '-f', 'mov', '-brand', 'mp42',
      '-movflags', 'frag_keyframe+empty_moov+default_base_moof',
      join(mediaDir, dir, 'clip.mp4'), '-y',
    ]);
  }

  // --- Update manifest ---
  await addFixtureEntry(entries, 'h264-1280x720-30fps-fmp4', join(mediaDir, 'h264-fmp4', 'clip.mp4'), 'H.264 + AAC fragmented MP4 1280x720 30fps 2s', 'http-fmp4');
  await addFixtureEntry(entries, 'h265-640x480-30fps-fmp4', join(mediaDir, 'h265-fmp4', 'clip.mp4'), 'H.265 + AAC fragmented MP4 640x480 30fps 2s', 'http-fmp4');
  await addFixtureEntry(entries, 'h264-http-fmp4-640x480', join(mediaDir, 'h264-http-fmp4', 'clip.mp4'), 'HTTP-fMP4 H.264/AAC 640x480 2s', 'http-fmp4');
  await addFixtureEntry(entries, 'h264-ws-fmp4-640x480', join(mediaDir, 'h264-ws-fmp4', 'clip.mp4'), 'WS-fMP4 H.264/AAC 640x480 2s', 'ws-fmp4');
  await addFixtureEntry(entries, 'mp3-44khz-fmp4', join(mediaDir, 'mp3-fmp4', 'clip.mp4'), 'MP3 audio in fMP4 container', 'http-fmp4');
  await addFixtureEntry(entries, 'aac-48khz-fmp4', join(mediaDir, 'aac-fmp4', 'audio.mp4'), 'AAC audio-only fMP4', 'http-fmp4');
  await addFixtureEntry(entries, 'hls-h264-fmp4-640x480', join(mediaDir, 'hls-h264-fmp4', 'master.m3u8'), 'HLS with H.264/AAC fMP4 segments 640x480', 'hls');
  await addFixtureEntry(entries, 'h264-flv-640x480', join(mediaDir, 'h264-flv', 'clip.flv'), 'HTTP-FLV H.264/AAC 640x480', 'http-flv');
  await addFixtureEntry(entries, 'h264-ws-flv-640x480', join(mediaDir, 'h264-flv', 'clip.flv'), 'WS-FLV H.264/AAC 640x480', 'ws-flv');
  await addFixtureEntry(entries, 'g711a-8khz-fmp4', join(mediaDir, 'g711a', 'clip.mp4'), 'G.711 A-law in fMP4 8kHz mono', 'http-fmp4');
  await addFixtureEntry(entries, 'g711u-8khz-fmp4', join(mediaDir, 'g711u', 'clip.mp4'), 'G.711 mu-law in fMP4 8kHz mono', 'http-fmp4');

  // Keep legacy synthetic fixtures from the testkit.
  const syntheticIds = new Set(['h264-1280x720-30fps-2s', 'aac-48khz-2ch-2s', 'g711a-8khz-1ch-500ms', 'h264-33bit-wrap', 'corrupt-flv']);
  for (const entry of entries) {
    if (syntheticIds.has(entry.id)) {
      entry.source.type = 'synthetic';
      entry.source.generator = entry.source.generator ?? 'cheetah-media-testkit';
      entry.source.url = undefined;
    }
  }

  manifest.schema_version = manifest.schema_version || '1.0';
  manifest.fixtures = entries;
  await writeFile(manifestPath, JSON.stringify(manifest, null, 2) + '\n');

  // Print summary
  let total = 0;
  for (const entry of entries) {
    if (entry.source?.type === 'download') {
      const rel = entry.source.url.replace(/^media\//, '');
      const filePath = join(mediaDir, rel);
      if (existsSync(filePath)) {
        total += await fileSize(filePath);
      }
    }
  }
  console.log(`[fixtures] generated ${entries.length} entries; total real fixture size ~${(total / 1024).toFixed(1)} KB`);
}

main().catch((err) => {
  console.error(err.message);
  process.exit(1);
});
