/**
 * Minimal intercom packetizer for captured G.711 audio.
 *
 * Encodes `AudioPacket` payloads into tiny RTP-like packets that can be sent
 * over WebSocket, WebRTC DataChannel, or any other transport the caller
 * provides. No network I/O is performed here.
 */

import type { AudioPacket } from './capture';

export interface IntercomPacket {
  /** Raw packet bytes ready for the wire. */
  readonly payload: Uint8Array;
  /** Wall-clock timestamp in milliseconds. */
  readonly timestampMs: number;
  /** RTP sequence number (16-bit, wraps). */
  readonly sequence: number;
  /** RTP timestamp in clock-rate units. */
  readonly rtpTimestamp: number;
  /** RTP SSRC. */
  readonly ssrc: number;
  /** RTP payload type. */
  readonly payloadType: number;
}

export interface IntercomPacketizerOptions {
  /** RTP payload type (default 0=PCMU, 8=PCMA). */
  readonly payloadType?: number;
  /** RTP SSRC (random 32-bit value if omitted). */
  readonly ssrc?: number;
  /** Optional callback invoked for each produced packet. */
  readonly onPacket?: (packet: IntercomPacket) => void;
}

export class IntercomPacketizer {
  private payloadType: number;
  private ssrc: number;
  private sequence = 0;
  private rtpTimestamp = 0;
  private onPacket: ((packet: IntercomPacket) => void) | undefined;

  constructor(options: IntercomPacketizerOptions = {}) {
    const payloadType = options.payloadType ?? 0;
    const ssrc = options.ssrc ?? generateSsrc();
    if (!Number.isFinite(payloadType) || payloadType < 0 || payloadType > 127 || payloadType % 1 !== 0) {
      throw new Error('payloadType must be an integer between 0 and 127');
    }
    if (!Number.isFinite(ssrc) || ssrc < 0) {
      throw new Error('ssrc must be a finite non-negative number');
    }
    this.payloadType = payloadType;
    this.ssrc = ssrc;
    this.onPacket = options.onPacket;
  }

  get currentSequence(): number {
    return this.sequence & 0xffff;
  }

  /** Feed one encoded audio frame and emit an `IntercomPacket`. */
  push(audioPacket: AudioPacket): void {
    if (!audioPacket || !audioPacket.payload || !(audioPacket.payload instanceof Uint8Array)) {
      throw new Error('push requires an AudioPacket with a Uint8Array payload');
    }
    const header = new Uint8Array(RTP_HEADER_SIZE);
    header[0] = 0x80; // V=2, P=0, X=0, CC=0
    header[1] = this.payloadType & 0x7f;
    writeUint16BE(header, 2, this.sequence & 0xffff);
    writeUint32BE(header, 4, this.rtpTimestamp >>> 0);
    writeUint32BE(header, 8, this.ssrc >>> 0);

    const packet = new Uint8Array(header.length + audioPacket.payload.length);
    packet.set(header);
    packet.set(audioPacket.payload, header.length);

    const produced: IntercomPacket = {
      payload: packet,
      timestampMs: audioPacket.timestampMs,
      sequence: this.sequence & 0xffff,
      rtpTimestamp: this.rtpTimestamp >>> 0,
      ssrc: this.ssrc >>> 0,
      payloadType: this.payloadType,
    };

    this.onPacket?.(produced);

    this.sequence += 1;
    this.rtpTimestamp += audioPacket.payload.length;
  }

  /** Reset sequence and timestamp counters. */
  reset(): void {
    this.sequence = 0;
    this.rtpTimestamp = 0;
  }
}

const RTP_HEADER_SIZE = 12;

function generateSsrc(): number {
  return Math.floor(Math.random() * 0xffffffff);
}

function writeUint16BE(buf: Uint8Array, offset: number, value: number): void {
  buf[offset] = (value >> 8) & 0xff;
  buf[offset + 1] = value & 0xff;
}

function writeUint32BE(buf: Uint8Array, offset: number, value: number): void {
  buf[offset] = (value >> 24) & 0xff;
  buf[offset + 1] = (value >> 16) & 0xff;
  buf[offset + 2] = (value >> 8) & 0xff;
  buf[offset + 3] = value & 0xff;
}
