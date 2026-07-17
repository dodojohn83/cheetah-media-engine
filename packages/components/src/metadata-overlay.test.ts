import { describe, it, expect, beforeEach } from 'vitest';
import {
  clearMetadataOverlay,
  parseMetadataPayload,
  renderMetadataOverlay,
  sanitizeStyle,
  type MetadataShape,
} from './metadata-overlay';

describe('parseMetadataPayload', () => {
  it('parses a JSON string with shapes', () => {
    const payload = JSON.stringify({
      shapes: [{ type: 'line', x1: 0.1, y1: 0.2, x2: 0.3, y2: 0.4 }],
    });
    const shapes = parseMetadataPayload(payload);
    expect(shapes).toHaveLength(1);
    expect(shapes[0]).toEqual({ type: 'line', x1: 0.1, y1: 0.2, x2: 0.3, y2: 0.4 });
  });

  it('parses a Uint8Array payload', () => {
    const payload = new TextEncoder().encode(
      JSON.stringify({ shapes: [{ type: 'rect', x: 0.1, y: 0.2, width: 0.3, height: 0.4 }] }),
    );
    const shapes = parseMetadataPayload(payload);
    expect(shapes).toHaveLength(1);
    expect(shapes[0]!.type).toBe('rect');
  });

  it('parses a raw object payload', () => {
    const payload = {
      shapes: [{ type: 'circle', cx: 0.5, cy: 0.5, r: 0.1, style: 'fill:red' }],
    };
    const shapes = parseMetadataPayload(payload);
    expect(shapes).toHaveLength(1);
    expect(shapes[0]!).toEqual({ type: 'circle', cx: 0.5, cy: 0.5, r: 0.1, style: 'fill:red' });
  });

  it('returns an empty array for invalid JSON', () => {
    expect(parseMetadataPayload('not json')).toEqual([]);
  });

  it('returns an empty array for missing shapes', () => {
    expect(parseMetadataPayload(JSON.stringify({}))).toEqual([]);
  });

  it('drops malformed shapes', () => {
    const payload = {
      shapes: [
        { type: 'line', x1: 0, y1: 0 }, // missing x2/y2
        { type: 'polygon', points: [[0, 0]] }, // not enough points
      ],
    };
    expect(parseMetadataPayload(payload)).toEqual([]);
  });

  it('supports all shape types', () => {
    const shapes: unknown[] = [
      { type: 'line', x1: 0.1, y1: 0.2, x2: 0.3, y2: 0.4 },
      { type: 'rect', x: 0.1, y: 0.2, width: 0.3, height: 0.4 },
      { type: 'circle', cx: 0.5, cy: 0.5, r: 0.1 },
      { type: 'polygon', points: [[0.1, 0.1], [0.2, 0.3], [0.3, 0.1]] },
      { type: 'text', x: 0.1, y: 0.1, text: 'hello' },
    ];
    const parsed = parseMetadataPayload({ shapes });
    expect(parsed).toHaveLength(5);
  });
});

describe('renderMetadataOverlay', () => {
  let svg: SVGSVGElement;

  beforeEach(() => {
    svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  });

  it('renders shapes as SVG elements', () => {
    const shapes: MetadataShape[] = [
      { type: 'line', x1: 0.1, y1: 0.2, x2: 0.3, y2: 0.4 },
      { type: 'rect', x: 0.1, y: 0.2, width: 0.3, height: 0.4, style: 'fill:red' },
    ];
    renderMetadataOverlay(shapes, svg);
    expect(svg.children.length).toBe(2);
    expect(svg.querySelector('line')).not.toBeNull();
    expect(svg.querySelector('rect')?.getAttribute('style')).toBe('fill: red');
  });

  it('replaces previous shapes', () => {
    renderMetadataOverlay([{ type: 'line', x1: 0, y1: 0, x2: 1, y2: 1 }], svg);
    renderMetadataOverlay([{ type: 'circle', cx: 0.5, cy: 0.5, r: 0.1 }], svg);
    expect(svg.children.length).toBe(1);
    expect(svg.querySelector('circle')).not.toBeNull();
    expect(svg.querySelector('line')).toBeNull();
  });

  it('clears shapes when given an empty list', () => {
    renderMetadataOverlay([{ type: 'line', x1: 0, y1: 0, x2: 1, y2: 1 }], svg);
    renderMetadataOverlay([], svg);
    expect(svg.children.length).toBe(0);
  });

  it('sets viewBox and preserveAspectRatio', () => {
    renderMetadataOverlay([{ type: 'line', x1: 0, y1: 0, x2: 1, y2: 1 }], svg);
    expect(svg.getAttribute('viewBox')).toBe('0 0 1 1');
    expect(svg.getAttribute('preserveAspectRatio')).toBe('xMidYMid meet');
  });

  it('renders text content safely', () => {
    const shapes: MetadataShape[] = [{ type: 'text', x: 0.1, y: 0.2, text: '<script>alert(1)</script>' }];
    renderMetadataOverlay(shapes, svg);
    const text = svg.querySelector('text');
    expect(text).not.toBeNull();
    expect(text?.textContent).toBe('<script>alert(1)</script>');
    expect(text?.innerHTML).not.toContain('<script>');
  });
});

describe('sanitizeStyle', () => {
  it('keeps safe SVG presentation properties', () => {
    const raw = 'fill: red; stroke: rgba(0,0,0,0.5); stroke-width: 0.002; opacity: 0.8';
    expect(sanitizeStyle(raw)).toBe(
      'fill: red; stroke: rgba(0,0,0,0.5); stroke-width: 0.002; opacity: 0.8',
    );
  });

  it('drops dangerous url/expression/javascript declarations', () => {
    const raw =
      "fill: red; background: url(https://attacker/); stroke: expression(alert(1)); color: javascript://";
    expect(sanitizeStyle(raw)).toBe('fill: red');
  });

  it('drops disallowed properties like pointer-events and z-index', () => {
    expect(sanitizeStyle('pointer-events: auto; z-index: 9999; fill: blue')).toBe('fill: blue');
  });

  it('returns undefined for oversized style strings', () => {
    expect(sanitizeStyle('fill: red'.repeat(300))).toBeUndefined();
  });

  it('drops declarations with malformed syntax', () => {
    expect(sanitizeStyle('fill-red; fill: blue;')).toBe('fill: blue');
  });
});

describe('clearMetadataOverlay', () => {
  it('removes all children', () => {
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    svg.appendChild(document.createElementNS('http://www.w3.org/2000/svg', 'line'));
    clearMetadataOverlay(svg);
    expect(svg.children.length).toBe(0);
  });
});
