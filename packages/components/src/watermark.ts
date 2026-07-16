/**
 * DOM-based local watermark overlay for the Cheetah player component.
 *
 * Supports text, image and HTML watermarks with optional tiling, motion
 * and ghost (pulsing opacity) effects. The overlay lives in the player
 * shadow DOM above the video surface and below status/control overlays.
 */

export interface WatermarkPosition {
  readonly x: number;
  readonly y: number;
}

export interface WatermarkBase {
  /** Optional stable identifier. */
  readonly id?: string | undefined;
  /** Horizontal position as a percentage of the container width (0..100). */
  readonly x?: number | undefined;
  /** Vertical position as a percentage of the container height (0..100). */
  readonly y?: number | undefined;
  /** CSS width, e.g. "120px" or "20%". */
  readonly width?: string | undefined;
  /** CSS height. */
  readonly height?: string | undefined;
  /** Opacity from 0 to 1. */
  readonly opacity?: number | undefined;
  /** Clockwise rotation in degrees. */
  readonly rotation?: number | undefined;
  /** Repeat the watermark across the container. */
  readonly tile?: boolean | undefined;
  /** Animate the watermark position. */
  readonly dynamic?: boolean | undefined;
  /** Pulse the watermark opacity. */
  readonly ghost?: boolean | undefined;
}

export interface TextWatermark extends WatermarkBase {
  readonly type: 'text';
  readonly content: string;
  readonly font?: string | undefined;
  readonly color?: string | undefined;
}

export interface ImageWatermark extends WatermarkBase {
  readonly type: 'image';
  readonly content: string;
}

export interface HtmlWatermark extends WatermarkBase {
  readonly type: 'html';
  readonly content: string;
}

export type Watermark = TextWatermark | ImageWatermark | HtmlWatermark;

export interface WatermarkOverlay {
  readonly root: HTMLDivElement;
  setWatermarks(watermarks: readonly Watermark[]): void;
  clear(): void;
}

const VALID_TYPES = new Set<string>(['text', 'image', 'html']);

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function isValidPosition(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function parseOpacity(value: unknown): number | undefined {
  if (typeof value !== 'number' || !Number.isFinite(value)) return undefined;
  return clamp(value, 0, 1);
}

function parseBoolean(value: unknown): boolean | undefined {
  return typeof value === 'boolean' ? value : undefined;
}

function parseString(value: unknown): string | undefined {
  if (typeof value !== 'string') return undefined;
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : undefined;
}

function parseBase(item: Record<string, unknown>): WatermarkBase | undefined {
  const x = isValidPosition(item.x) ? clamp(item.x, 0, 100) : 0;
  const y = isValidPosition(item.y) ? clamp(item.y, 0, 100) : 0;
  const width = parseString(item.width);
  const height = parseString(item.height);
  const opacity = parseOpacity(item.opacity);
  const rotation = isValidPosition(item.rotation) ? item.rotation : undefined;
  const tile = parseBoolean(item.tile);
  const dynamic = parseBoolean(item.dynamic);
  const ghost = parseBoolean(item.ghost);

  return {
    x,
    y,
    ...(width !== undefined ? { width } : {}),
    ...(height !== undefined ? { height } : {}),
    ...(opacity !== undefined ? { opacity } : {}),
    ...(rotation !== undefined ? { rotation } : {}),
    ...(tile !== undefined ? { tile } : {}),
    ...(dynamic !== undefined ? { dynamic } : {}),
    ...(ghost !== undefined ? { ghost } : {}),
  };
}

function parseWatermark(item: Record<string, unknown>): Watermark | undefined {
  if (item === null || typeof item !== 'object') return undefined;
  const base = parseBase(item);
  const id = parseString(item.id);
  const type = parseString(item.type);
  const content = parseString(item.content);

  if (!type || !VALID_TYPES.has(type) || !content) return undefined;
  if (type === 'image' && isDangerousUrl(content)) return undefined;

  const common = {
    ...base,
    type,
    content,
    ...(id !== undefined ? { id } : {}),
  } as WatermarkBase & { type: 'text' | 'image' | 'html'; content: string };

  if (common.type === 'text') {
    return {
      ...common,
      type: 'text',
      content: common.content,
      ...(parseString(item.font) !== undefined ? { font: parseString(item.font) } : {}),
      ...(parseString(item.color) !== undefined ? { color: parseString(item.color) } : {}),
    } as TextWatermark;
  }

  return common as ImageWatermark | HtmlWatermark;
}

/**
 * Parse the `watermarks` attribute value (a JSON array) into validated
 * watermark objects. Returns `undefined` for invalid top-level input.
 */
export function parseWatermarks(value: string | null): Watermark[] | undefined {
  if (!value) return undefined;
  try {
    const parsed = JSON.parse(value) as unknown;
    if (!Array.isArray(parsed)) return undefined;
    const result: Watermark[] = [];
    for (const item of parsed) {
      const wm = parseWatermark(item as Record<string, unknown>);
      if (wm) result.push(wm);
    }
    return result;
  } catch {
    return undefined;
  }
}

const ALLOWED_TAGS = new Set<string>([
  'a',
  'b',
  'br',
  'div',
  'em',
  'font',
  'h1',
  'h2',
  'h3',
  'h4',
  'h5',
  'h6',
  'i',
  'img',
  'li',
  'ol',
  'p',
  's',
  'small',
  'span',
  'strike',
  'strong',
  'sub',
  'sup',
  'table',
  'tbody',
  'td',
  'th',
  'thead',
  'tr',
  'u',
  'ul',
]);

const ALLOWED_ATTRIBUTES = new Set<string>([
  'alt',
  'class',
  'colspan',
  'height',
  'href',
  'rowspan',
  'src',
  'style',
  'target',
  'title',
  'width',
]);

function isDangerousUrl(value: string): boolean {
  // Browsers ignore tabs, newlines and control characters inside a URL scheme,
  // so normalize them before checking the scheme prefix.
  const normalized = value.replace(/[\x00-\x20]/g, '').toLowerCase();
  if (normalized.startsWith('javascript:') || normalized.startsWith('vbscript:')) return true;
  if (
    normalized.startsWith('data:text/html') ||
    normalized.startsWith('data:image/svg+xml')
  ) {
    return true;
  }
  return false;
}

const DANGEROUS_STYLE_PROPERTIES = new Set<string>(['behavior', '-moz-binding', 'expression']);
const DANGEROUS_STYLE_VALUE_PATTERN = /expression\s*\(|behaviour\s*\(|vbscript:|javascript:/i;
const URL_VALUE_PATTERN = /url\s*\(\s*["']?([^\)"']+)["']?\s*\)/gi;

function styleUrlIsSafe(url: string): boolean {
  const trimmed = url.trim().toLowerCase();
  return !(
    trimmed.startsWith('javascript:') ||
    trimmed.startsWith('vbscript:') ||
    trimmed.startsWith('data:text/html') ||
    trimmed.startsWith('data:image/svg+xml')
  );
}

/**
 * Sanitize an inline style declaration by dropping dangerous properties and
 * any `url(...)` references that contain executable or HTML data schemes.
 */
function sanitizeStyle(value: string): string | undefined {
  const declarations: string[] = [];
  const parts = value.split(';');
  for (const part of parts) {
    const colon = part.indexOf(':');
    if (colon === -1) continue;
    const property = part.slice(0, colon).trim().toLowerCase();
    const propertyValue = part.slice(colon + 1).trim();
    if (!property || DANGEROUS_STYLE_PROPERTIES.has(property)) continue;
    if (DANGEROUS_STYLE_VALUE_PATTERN.test(propertyValue)) continue;

    // Reject url(...) values with dangerous schemes.
    let safeValue = propertyValue;
    let match: RegExpExecArray | null;
    let unsafeUrl = false;
    URL_VALUE_PATTERN.lastIndex = 0;
    while ((match = URL_VALUE_PATTERN.exec(propertyValue)) !== null) {
      if (!styleUrlIsSafe(match[1]!)) {
        unsafeUrl = true;
        break;
      }
    }
    if (unsafeUrl) continue;

    // Remove any comments that older IE might misparse.
    safeValue = safeValue.replace(/\/\*[\s\S]*?\*\//g, '');
    if (!safeValue) continue;

    declarations.push(`${property}:${safeValue}`);
  }
  return declarations.length > 0 ? declarations.join(';') : undefined;
}

function sanitizeAttribute(name: string, value: string): string | undefined {
  const lowerName = name.toLowerCase();
  if (lowerName.startsWith('on')) return undefined;
  if (!ALLOWED_ATTRIBUTES.has(lowerName)) return undefined;
  if ((lowerName === 'href' || lowerName === 'src') && isDangerousUrl(value)) return undefined;
  if (lowerName === 'style') {
    return sanitizeStyle(value);
  }
  return value;
}

function sanitizeNode(node: Node): Node | undefined {
  if (node.nodeType === Node.TEXT_NODE) {
    return document.createTextNode(node.textContent ?? '');
  }
  if (node.nodeType !== Node.ELEMENT_NODE) {
    return undefined;
  }
  const element = node as Element;
  const tag = element.tagName.toLowerCase();
  if (!ALLOWED_TAGS.has(tag)) {
    // Dangerous containers such as <script> and <style> are dropped entirely;
    // other unknown tags have their safe children flattened into the parent.
    if (tag === 'script' || tag === 'style') {
      return undefined;
    }
    const fragment = document.createDocumentFragment();
    for (const child of Array.from(element.childNodes)) {
      const sanitized = sanitizeNode(child);
      if (sanitized) fragment.appendChild(sanitized);
    }
    return fragment.childNodes.length > 0 ? fragment : undefined;
  }

  const safe = document.createElement(tag);
  for (const attr of Array.from(element.attributes)) {
    const value = sanitizeAttribute(attr.name, attr.value);
    if (value !== undefined) {
      safe.setAttribute(attr.name, value);
    }
  }
  for (const child of Array.from(element.childNodes)) {
    const sanitized = sanitizeNode(child);
    if (sanitized) safe.appendChild(sanitized);
  }
  return safe;
}

/**
 * Sanitize a raw HTML string for use in an HTML watermark. Removes scripts,
 * event handlers, dangerous URLs and non-allowlisted tags/attributes.
 */
export function sanitizeHtml(html: string): DocumentFragment {
  const parser = new DOMParser();
  const doc = parser.parseFromString(html, 'text/html');
  const fragment = document.createDocumentFragment();
  for (const child of Array.from(doc.body.childNodes)) {
    const sanitized = sanitizeNode(child);
    if (sanitized) fragment.appendChild(sanitized);
  }
  return fragment;
}

function createWatermarkItem(watermark: Watermark, index: number): HTMLElement {
  const el: HTMLElement =
    watermark.type === 'image'
      ? document.createElement('img')
      : document.createElement('div');

  el.className = 'watermark-item';
  if (watermark.id) {
    el.dataset.watermarkId = watermark.id;
  } else {
    el.dataset.watermarkIndex = String(index);
  }

  if (watermark.type === 'text') {
    el.textContent = watermark.content;
    if (watermark.font) el.style.font = watermark.font;
    if (watermark.color) el.style.color = watermark.color;
  } else if (watermark.type === 'image') {
    const img = el as HTMLImageElement;
    img.src = watermark.content;
    img.alt = '';
    // Decorative watermark images do not need a screen-reader announcement.
    img.setAttribute('aria-hidden', 'true');
  } else {
    el.appendChild(sanitizeHtml(watermark.content));
  }

  const opacity = watermark.opacity;
  if (opacity !== undefined) {
    el.style.opacity = String(opacity);
  }

  const rotation = watermark.rotation;
  if (rotation !== undefined) {
    el.style.transform = `rotate(${rotation}deg)`;
  }

  if (watermark.width) {
    el.style.width = watermark.width;
  }
  if (watermark.height) {
    el.style.height = watermark.height;
  }

  if (watermark.dynamic) {
    el.classList.add('watermark-dynamic');
  }
  if (watermark.ghost) {
    el.classList.add('watermark-ghost');
  }

  return el;
}

function createTiledContainer(watermark: Watermark, index: number): HTMLDivElement {
  const wrapper = document.createElement('div');
  wrapper.className = 'watermark-tile-container';
  if (watermark.id) {
    wrapper.dataset.watermarkId = watermark.id;
  } else {
    wrapper.dataset.watermarkIndex = String(index);
  }

  const tileCount = 12;
  for (let i = 0; i < tileCount; i += 1) {
    const tile = createWatermarkItem(watermark, index);
    tile.classList.add('watermark-tile-item');
    wrapper.appendChild(tile);
  }

  return wrapper;
}

function createWatermarkNode(watermark: Watermark, index: number): HTMLElement {
  if (watermark.tile) {
    return createTiledContainer(watermark, index);
  }

  const el = createWatermarkItem(watermark, index);
  el.style.left = `${watermark.x ?? 0}%`;
  el.style.top = `${watermark.y ?? 0}%`;
  return el;
}

class WatermarkOverlayImpl implements WatermarkOverlay {
  readonly root: HTMLDivElement;

  constructor() {
    this.root = document.createElement('div');
    this.root.className = 'watermark-layer';
    this.root.setAttribute('part', 'watermark-layer');
    this.root.setAttribute('aria-hidden', 'true');
  }

  setWatermarks(watermarks: readonly Watermark[]): void {
    this.clear();
    for (const [index, watermark] of watermarks.entries()) {
      this.root.appendChild(createWatermarkNode(watermark, index));
    }
  }

  clear(): void {
    while (this.root.firstChild) {
      this.root.removeChild(this.root.firstChild);
    }
  }
}

export function createWatermarkOverlay(): WatermarkOverlay {
  return new WatermarkOverlayImpl();
}
