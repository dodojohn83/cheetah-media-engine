//! Server-side metadata overlay rendering on top of the video element.
//!
//! Overlay payloads are JSON objects with a `shapes` array. Each shape carries
//! normalized [0,1] coordinates and a `style` string that is written verbatim to
//! the SVG element's `style` attribute.

export interface MetadataShapeLine {
  readonly type: 'line';
  readonly x1: number;
  readonly y1: number;
  readonly x2: number;
  readonly y2: number;
  readonly style?: string | undefined;
}

export interface MetadataShapeRect {
  readonly type: 'rect';
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly style?: string | undefined;
}

export interface MetadataShapeCircle {
  readonly type: 'circle';
  readonly cx: number;
  readonly cy: number;
  readonly r: number;
  readonly style?: string | undefined;
}

export interface MetadataShapePolygon {
  readonly type: 'polygon';
  readonly points: readonly (readonly [number, number])[];
  readonly style?: string | undefined;
}

export interface MetadataShapeText {
  readonly type: 'text';
  readonly x: number;
  readonly y: number;
  readonly text: string;
  readonly style?: string | undefined;
}

export type MetadataShape =
  | MetadataShapeLine
  | MetadataShapeRect
  | MetadataShapeCircle
  | MetadataShapePolygon
  | MetadataShapeText;

function isNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function isString(value: unknown): value is string {
  return typeof value === 'string';
}

function isArray(value: unknown): value is readonly unknown[] {
  return Array.isArray(value);
}

function toNumber(value: unknown): number | undefined {
  if (isNumber(value)) return value;
  if (isString(value)) {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return undefined;
}

function parsePoint(point: unknown): [number, number] | undefined {
  if (!isArray(point) || point.length < 2) return undefined;
  const x = toNumber(point[0]);
  const y = toNumber(point[1]);
  if (x === undefined || y === undefined) return undefined;
  return [x, y];
}

function parseStyle(raw: unknown): string | undefined {
  if (raw === undefined || raw === null) return undefined;
  if (isString(raw)) return raw;
  return undefined;
}

function parseShape(raw: unknown): MetadataShape | undefined {
  if (raw === null || typeof raw !== 'object') return undefined;
  const obj = raw as Record<string, unknown>;
  const type = obj.type;
  const style = parseStyle(obj.style);

  switch (type) {
    case 'line': {
      const x1 = toNumber(obj.x1);
      const y1 = toNumber(obj.y1);
      const x2 = toNumber(obj.x2);
      const y2 = toNumber(obj.y2);
      if (x1 === undefined || y1 === undefined || x2 === undefined || y2 === undefined) {
        return undefined;
      }
      return { type: 'line', x1, y1, x2, y2, style };
    }
    case 'rect': {
      const x = toNumber(obj.x);
      const y = toNumber(obj.y);
      const width = toNumber(obj.width);
      const height = toNumber(obj.height);
      if (x === undefined || y === undefined || width === undefined || height === undefined) {
        return undefined;
      }
      return { type: 'rect', x, y, width, height, style };
    }
    case 'circle': {
      const cx = toNumber(obj.cx);
      const cy = toNumber(obj.cy);
      const r = toNumber(obj.r);
      if (cx === undefined || cy === undefined || r === undefined) return undefined;
      return { type: 'circle', cx, cy, r, style };
    }
    case 'polygon': {
      if (!isArray(obj.points)) return undefined;
      const points = obj.points.map(parsePoint).filter((p): p is [number, number] => p !== undefined);
      if (points.length < 3) return undefined;
      return { type: 'polygon', points, style };
    }
    case 'text': {
      const x = toNumber(obj.x);
      const y = toNumber(obj.y);
      const text = isString(obj.text) ? obj.text : String(obj.text ?? '');
      if (x === undefined || y === undefined) return undefined;
      return { type: 'text', x, y, text, style };
    }
    default:
      return undefined;
  }
}

/** Parse a JSON metadata payload into a list of validated shapes. */
export function parseMetadataPayload(value: unknown): MetadataShape[] {
  let parsed: unknown;
  if (value instanceof Uint8Array) {
    try {
      const text = new TextDecoder().decode(value);
      parsed = JSON.parse(text);
    } catch {
      return [];
    }
  } else if (isString(value)) {
    try {
      parsed = JSON.parse(value);
    } catch {
      return [];
    }
  } else {
    parsed = value;
  }

  if (parsed === null || typeof parsed !== 'object') return [];
  const shapes = (parsed as Record<string, unknown>).shapes;
  if (!isArray(shapes)) return [];

  return shapes.map(parseShape).filter((s): s is MetadataShape => s !== undefined);
}

function createSvgElement<K extends keyof SVGElementTagNameMap>(
  tag: K,
): SVGElementTagNameMap[K] {
  return document.createElementNS('http://www.w3.org/2000/svg', tag);
}

function setStyle(element: SVGElement, style: string | undefined): void {
  if (style !== undefined && style.length > 0) {
    element.setAttribute('style', style);
  }
}

/** Render a list of shapes into an SVG element, replacing any previous content. */
export function renderMetadataOverlay(shapes: readonly MetadataShape[], svg: SVGSVGElement): void {
  while (svg.lastChild) {
    svg.removeChild(svg.lastChild);
  }

  if (shapes.length === 0) return;

  svg.setAttribute('viewBox', '0 0 1 1');
  svg.setAttribute('preserveAspectRatio', 'xMidYMid meet');

  for (const shape of shapes) {
    switch (shape.type) {
      case 'line': {
        const el = createSvgElement('line');
        el.setAttribute('x1', String(shape.x1));
        el.setAttribute('y1', String(shape.y1));
        el.setAttribute('x2', String(shape.x2));
        el.setAttribute('y2', String(shape.y2));
        setStyle(el, shape.style);
        svg.appendChild(el);
        break;
      }
      case 'rect': {
        const el = createSvgElement('rect');
        el.setAttribute('x', String(shape.x));
        el.setAttribute('y', String(shape.y));
        el.setAttribute('width', String(shape.width));
        el.setAttribute('height', String(shape.height));
        setStyle(el, shape.style);
        svg.appendChild(el);
        break;
      }
      case 'circle': {
        const el = createSvgElement('circle');
        el.setAttribute('cx', String(shape.cx));
        el.setAttribute('cy', String(shape.cy));
        el.setAttribute('r', String(shape.r));
        setStyle(el, shape.style);
        svg.appendChild(el);
        break;
      }
      case 'polygon': {
        const el = createSvgElement('polygon');
        const points = shape.points.map(([x, y]) => `${x},${y}`).join(' ');
        el.setAttribute('points', points);
        setStyle(el, shape.style);
        svg.appendChild(el);
        break;
      }
      case 'text': {
        const el = createSvgElement('text');
        el.setAttribute('x', String(shape.x));
        el.setAttribute('y', String(shape.y));
        el.textContent = shape.text;
        setStyle(el, shape.style);
        svg.appendChild(el);
        break;
      }
      default:
        break;
    }
  }
}

/** Clear all shapes from the overlay SVG. */
export function clearMetadataOverlay(svg: SVGSVGElement): void {
  while (svg.lastChild) {
    svg.removeChild(svg.lastChild);
  }
}
