import { describe, expect, it } from 'vitest';
import { detectProtocol, protocolSupportedByMseSession } from './playback-session';

describe('playback session protocol detection', () => {
  it('detects HLS playlists', () => {
    expect(detectProtocol('https://cdn.example/live/index.m3u8')).toBe('hls');
    expect(detectProtocol('https://cdn.example/ll-hls/index.m3u8')).toBe('ll-hls');
  });

  it('detects fMP4 and FLV by extension and scheme', () => {
    expect(detectProtocol('https://cdn.example/a.mp4')).toBe('http-fmp4');
    expect(detectProtocol('wss://cdn.example/a.mp4')).toBe('ws-fmp4');
    expect(detectProtocol('https://cdn.example/a.flv')).toBe('http-flv');
    expect(detectProtocol('ws://cdn.example/a.flv')).toBe('ws-flv');
  });

  it('honors explicit protocol hints', () => {
    expect(detectProtocol('https://cdn.example/stream', 'http-fmp4')).toBe('http-fmp4');
  });

  it('marks MSE-native and FLV-transmux protocols as supported by the session', () => {
    expect(protocolSupportedByMseSession('http-fmp4')).toBe(true);
    expect(protocolSupportedByMseSession('hls')).toBe(true);
    expect(protocolSupportedByMseSession('http-flv')).toBe(true);
    expect(protocolSupportedByMseSession('ws-flv')).toBe(true);
    expect(protocolSupportedByMseSession('ws-annexb')).toBe(false);
  });
});
