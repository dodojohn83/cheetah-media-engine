/**
 * GB28181 PTZ command encoder.
 *
 * This module only generates the `PTZCmd` hex payload; it does not send it to
 * the device. The application is responsible for forwarding the payload through
 * its signaling channel.
 */

/** Pan/tilt/zoom actions supported by the encoder. */
export type PtzMoveAction =
  | 'stop'
  | 'up'
  | 'down'
  | 'left'
  | 'right'
  | 'upLeft'
  | 'upRight'
  | 'downLeft'
  | 'downRight'
  | 'zoomIn'
  | 'zoomOut';

export type PtzPresetAction = 'presetSet' | 'presetCall' | 'presetDel';

export type PtzAction = PtzMoveAction | PtzPresetAction;

export interface PtzSpeeds {
  /** Pan/tilt horizontal speed, 0~255. */
  readonly horizontal?: number;
  /** Pan/tilt vertical speed, 0~255. */
  readonly vertical?: number;
  /** Lens zoom speed, 0~255 (only high 4 bits are used by GB28181). */
  readonly zoom?: number;
}

export interface PtzCommand {
  readonly action: PtzAction;
  readonly speeds?: PtzSpeeds;
  /** Preset point index, 1~255. Required for preset actions. */
  readonly presetPoint?: number;
  /** Device channel address low byte, defaults to 1. */
  readonly channel?: number;
}

// Bit-mapped command codes used by the 8-byte movement PTZCmd.
// Pan/tilt bits can be combined; zoom bits are separate.
const MOVE_CODES: Record<PtzMoveAction, number> = {
  stop: 0x00,
  right: 0x01,
  left: 0x02,
  down: 0x04,
  downRight: 0x05, // 0x04 | 0x01
  downLeft: 0x06, // 0x04 | 0x02
  up: 0x08,
  upRight: 0x09, // 0x08 | 0x01
  upLeft: 0x0a, // 0x08 | 0x02
  zoomIn: 0x10,
  zoomOut: 0x20,
};

const PRESET_CODES: Record<PtzPresetAction, number> = {
  presetSet: 0x81,
  presetCall: 0x82,
  presetDel: 0x83,
};

function clampByte(value: number | undefined): number {
  if (value === undefined || !Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(255, Math.round(value)));
}

function toHex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('').toUpperCase();
}

function checksum(bytes: Uint8Array): number {
  let sum = 0;
  for (let i = 0; i < bytes.length - 1; i += 1) {
    sum += bytes[i]!;
  }
  return sum & 0xff;
}

function isMoveAction(action: string): action is PtzMoveAction {
  return Object.prototype.hasOwnProperty.call(MOVE_CODES, action);
}

function isPresetAction(action: string): action is PtzPresetAction {
  return Object.prototype.hasOwnProperty.call(PRESET_CODES, action);
}

/**
 * Generate an 8-byte GB28181 PTZCmd as an upper-case hex string.
 *
 * Movement commands use the form:
 *   A5 0F <channel> <cmd> <hs> <vs> <z> <checksum>
 * where <z> is (zoomSpeed & 0xF0) with the low nibble cleared.
 *
 * Preset commands use the form:
 *   A5 0F <channel> <preset-cmd> 00 <point> 00 <checksum>
 */
export function createGb28181PtzCmd(command: PtzCommand): string {
  if (!command || typeof command !== 'object') {
    throw new Error('command must be an object');
  }
  if (typeof command.action !== 'string') {
    throw new Error('command.action must be a string');
  }
  const { action, channel = 1, presetPoint } = command;
  const rawSpeeds = command.speeds as unknown;
  if (rawSpeeds !== undefined && (rawSpeeds === null || typeof rawSpeeds !== 'object')) {
    throw new Error('command.speeds must be an object');
  }
  const speeds: PtzSpeeds = (command.speeds as PtzSpeeds | undefined) ?? {};

  if (!Number.isFinite(channel) || channel < 0 || channel > 255) {
    throw new Error(`Invalid PTZ channel ${channel}`);
  }
  const address = clampByte(channel);

  const buf = new Uint8Array(8);
  buf[0] = 0xa5;
  // Byte 2 is the fixed "version + check" combination used by GB28181 PTZCmd.
  // It is computed as (0xA >> 4 + 0xA & 0xF + address-high) % 16; with address
  // high nibble 0, this is 0x0F.
  buf[1] = 0x0f;
  buf[2] = address;

  if (isMoveAction(action)) {
    const horizontal = clampByte(speeds.horizontal);
    const vertical = clampByte(speeds.vertical);
    const zoom = clampByte(speeds.zoom);

    buf[3] = MOVE_CODES[action];
    buf[4] = horizontal;
    buf[5] = vertical;
    // GB28181 only uses the top 4 bits of the zoom byte; low nibble is 0.
    buf[6] = zoom & 0xf0;
  } else if (isPresetAction(action)) {
    const point = clampByte(presetPoint);
    if (point === 0) {
      throw new Error(`presetPoint is required for ${action} and must be 1~255`);
    }
    buf[3] = PRESET_CODES[action];
    buf[4] = 0x00;
    buf[5] = point;
    buf[6] = 0x00;
  } else {
    throw new Error(`Unsupported PTZ action: ${action}`);
  }

  buf[7] = checksum(buf);
  return toHex(buf);
}
