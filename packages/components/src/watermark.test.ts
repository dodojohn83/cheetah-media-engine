import { describe, it, expect, beforeEach } from 'vitest';
import {
  createWatermarkOverlay,
  parseWatermarks,
  sanitizeHtml,
  type TextWatermark,
  type ImageWatermark,
  type HtmlWatermark,
} from './watermark';

describe('parseWatermarks', () => {
  it('returns undefined for empty or invalid input', () => {
    expect(parseWatermarks(null)).toBeUndefined();
    expect(parseWatermarks('')).toBeUndefined();
    expect(parseWatermarks('not-json')).toBeUndefined();
    expect(parseWatermarks('{}')).toBeUndefined();
  });

  it('parses a valid watermark list', () => {
    const result = parseWatermarks(
      JSON.stringify([
        { type: 'text', content: 'hello', x: 10, y: 20, opacity: 0.5, dynamic: true },
        { type: 'image', content: 'data:image/png;base64,', tile: true },
        { type: 'html', content: '<b>warn</b>', ghost: true },
      ]),
    );

    expect(result).toHaveLength(3);
    expect(result![0]!).toMatchObject({ type: 'text', content: 'hello', x: 10, y: 20, opacity: 0.5, dynamic: true });
    expect(result![1]!).toMatchObject({ type: 'image', content: 'data:image/png;base64,', tile: true });
    expect(result![2]!).toMatchObject({ type: 'html', content: '<b>warn</b>', ghost: true });
  });

  it('drops malformed entries and clamps values', () => {
    const result = parseWatermarks(
      JSON.stringify([
        { type: 'text', content: 'ok' },
        { type: 'unknown', content: 'bad' },
        { type: 'text', content: '' },
        { type: 'text', content: 'out', x: 150, opacity: -1 },
      ]),
    );

    expect(result).toHaveLength(2);
    expect(result![0]!).toMatchObject({ type: 'text', content: 'ok', x: 0, y: 0 });
    expect(result![1]!).toMatchObject({ type: 'text', content: 'out', x: 100, opacity: 0 });
  });
});

describe('createWatermarkOverlay', () => {
  let overlay: ReturnType<typeof createWatermarkOverlay>;

  beforeEach(() => {
    overlay = createWatermarkOverlay();
  });

  it('creates a watermark layer root', () => {
    expect(overlay.root.className).toBe('watermark-layer');
    expect(overlay.root.getAttribute('part')).toBe('watermark-layer');
  });

  it('renders text watermarks', () => {
    overlay.setWatermarks([{ type: 'text', content: 'hello', x: 10, y: 20 }]);
    const items = overlay.root.querySelectorAll('.watermark-item');
    expect(items.length).toBe(1);
    expect(items[0]!.textContent).toBe('hello');
    expect((items[0] as HTMLElement).style.left).toBe('10%');
    expect((items[0] as HTMLElement).style.top).toBe('20%');
  });

  it('renders image watermarks', () => {
    const dataUrl = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==';
    overlay.setWatermarks([{ type: 'image', content: dataUrl, width: '120px' }]);
    const items = overlay.root.querySelectorAll('.watermark-item');
    expect(items.length).toBe(1);
    expect(items[0]!.tagName.toLowerCase()).toBe('img');
    expect((items[0] as HTMLImageElement).src).toBe(dataUrl);
    expect((items[0] as HTMLElement).style.width).toBe('120px');
  });

  it('renders html watermarks', () => {
    overlay.setWatermarks([{ type: 'html', content: '<span class="mark">X</span>' }]);
    const items = overlay.root.querySelectorAll('.watermark-item');
    expect(items.length).toBe(1);
    expect(items[0]!.innerHTML).toBe('<span class="mark">X</span>');
  });

  it('applies opacity, rotation, dynamic and ghost classes', () => {
    overlay.setWatermarks([
      { type: 'text', content: 'mark', opacity: 0.4, rotation: 45, dynamic: true, ghost: true },
    ]);
    const item = overlay.root.querySelector('.watermark-item') as HTMLElement;
    expect(item.style.opacity).toBe('0.4');
    expect(item.style.transform).toBe('rotate(45deg)');
    expect(item.classList.contains('watermark-dynamic')).toBe(true);
    expect(item.classList.contains('watermark-ghost')).toBe(true);
  });

  it('renders tiled watermarks with repeated items', () => {
    overlay.setWatermarks([{ type: 'text', content: ' tiled', tile: true }]);
    const containers = overlay.root.querySelectorAll('.watermark-tile-container');
    expect(containers.length).toBe(1);
    const tiles = containers[0]!.querySelectorAll('.watermark-tile-item');
    expect(tiles.length).toBe(12);
  });

  it('clears previous watermarks when set again', () => {
    overlay.setWatermarks([{ type: 'text', content: 'first' }]);
    expect(overlay.root.querySelectorAll('.watermark-item').length).toBe(1);
    overlay.setWatermarks([{ type: 'text', content: 'second' }]);
    const items = overlay.root.querySelectorAll('.watermark-item');
    expect(items.length).toBe(1);
    expect(items[0]!.textContent).toBe('second');
  });

  it('clears all watermarks on clear()', () => {
    overlay.setWatermarks([{ type: 'text', content: 'first' }]);
    overlay.clear();
    expect(overlay.root.querySelectorAll('.watermark-item').length).toBe(0);
  });
});

describe('Watermark type compatibility', () => {
  it('accepts discriminated text watermark with font and color', () => {
    const wm: TextWatermark = {
      type: 'text',
      content: 'hello',
      font: '16px sans-serif',
      color: '#ff0000',
      x: 50,
      y: 50,
      opacity: 0.5,
      rotation: 30,
      tile: false,
      dynamic: true,
      ghost: false,
    };
    expect(wm.type).toBe('text');
  });

  it('accepts image watermark', () => {
    const wm: ImageWatermark = { type: 'image', content: 'data:image/png,', x: 0, y: 0 };
    expect(wm.type).toBe('image');
  });

  it('accepts html watermark', () => {
    const wm: HtmlWatermark = { type: 'html', content: '<p>warning</p>', opacity: 0.3, ghost: true };
    expect(wm.type).toBe('html');
  });
});

describe('sanitizeHtml', () => {
  it('keeps safe formatting markup', () => {
    const fragment = sanitizeHtml('<p class="note">Hello <b>world</b></p>');
    const div = document.createElement('div');
    div.appendChild(fragment);
    expect(div.innerHTML).toBe('<p class="note">Hello <b>world</b></p>');
  });

  it("removes script tags and event handlers", () => {
    const fragment = sanitizeHtml('<p onclick="alert(1)">x</p><script>/* should not run */</script><span onmouseover="bad">y</span>');
    const div = document.createElement('div');
    div.appendChild(fragment);
    expect(div.querySelector('script')).toBeNull();
    expect(div.querySelector('p')?.getAttribute('onclick')).toBeNull();
    expect(div.querySelector('span')?.getAttribute('onmouseover')).toBeNull();
    expect(div.textContent).toBe('xy');
  });

  it('removes dangerous URLs', () => {
    const fragment = sanitizeHtml('<a href="javascript:alert(1)">link</a><img src="javascript:alert(2)">');
    const div = document.createElement('div');
    div.appendChild(fragment);
    expect(div.querySelector('a')?.getAttribute('href')).toBeNull();
    expect(div.querySelector('img')?.getAttribute('src')).toBeNull();
  });

  it('rejects dangerous URLs with embedded whitespace or control characters', () => {
    const fragment = sanitizeHtml('<a href="java\tscript:alert(1)">link</a><a href="\x01javascript:alert(2)">link2</a>');
    const div = document.createElement('div');
    div.appendChild(fragment);
    const links = div.querySelectorAll('a');
    expect(links[0]!.getAttribute('href')).toBeNull();
    expect(links[1]!.getAttribute('href')).toBeNull();
  });

  it('sanitizes inline styles', () => {
    const fragment = sanitizeHtml('<span style="color:red; background:url(javascript:alert(1)); -moz-binding:url(x); behavior:url(x); width:expression(alert(2)); font-weight:bold">x</span>');
    const div = document.createElement('div');
    div.appendChild(fragment);
    const span = div.querySelector('span');
    const style = span?.getAttribute('style') ?? '';
    expect(style).toContain('color');
    expect(style).toContain('font-weight');
    expect(style).not.toContain('background');
    expect(style).not.toContain('-moz-binding');
    expect(style).not.toContain('behavior');
    expect(style).not.toContain('expression');
  });

  it('removes non-allowlisted tags and attributes', () => {
    const fragment = sanitizeHtml('<x-foo data-x="1"><span data-x="1" unknown-attr="2">safe</span></x-foo>');
    const div = document.createElement('div');
    div.appendChild(fragment);
    expect(div.querySelector('x-foo')).toBeNull();
    expect(div.querySelector('span')?.getAttribute('data-x')).toBeNull();
    expect(div.querySelector('span')?.getAttribute('unknown-attr')).toBeNull();
    expect(div.textContent).toBe('safe');
  });
});
