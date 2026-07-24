/**
 * WebGL2 renderer for I420 / NV12 / RGBA video frames.
 *
 * Uploads tightly packed planes copied from the frame, applies the YUV -> RGB
 * color matrix in a fragment shader and renders a viewport-fitted quad. Handles
 * WebGL context loss by detecting `isContextLost()` and throwing a typed error.
 */

import type {
  RenderFrame,
  Renderer,
  RendererConfig,
  RendererMetrics,
  MutableRendererMetrics,
  SnapshotOptions,
  SnapshotResult,
} from './types';
import { RendererError } from './types';
import { RendererSurface } from './surface';
import { buildYuvToRgbCoeffs, resolveColorSpace } from './color';
import { validateSnapshotEncoderOptions } from './snapshot-encoder';

const VERTEX_SHADER = `#version 300 es
in vec2 a_position;
in vec2 a_texCoord;
uniform mat3 u_viewMatrix;
out vec2 v_texCoord;
void main() {
  vec3 p = u_viewMatrix * vec3(a_position, 1.0);
  gl_Position = vec4(p.xy, 0.0, 1.0);
  v_texCoord = a_texCoord;
}`;

const FRAG_RGBA = `#version 300 es
precision mediump float;
uniform sampler2D u_texture;
in vec2 v_texCoord;
out vec4 fragColor;
void main() {
  fragColor = texture(u_texture, v_texCoord);
}`;

const FRAG_YUV = `#version 300 es
precision mediump float;
uniform sampler2D u_y;
uniform sampler2D u_u;
uniform sampler2D u_v;
uniform mat3 u_matrix;
uniform vec3 u_offset;
in vec2 v_texCoord;
out vec4 fragColor;
void main() {
  float y = texture(u_y, v_texCoord).r;
  float u = texture(u_u, v_texCoord).r - 0.5;
  float v = texture(u_v, v_texCoord).r - 0.5;
  vec3 rgb = u_matrix * vec3(y, u, v) + u_offset;
  fragColor = vec4(clamp(rgb, 0.0, 1.0), 1.0);
}`;

const FRAG_NV12 = `#version 300 es
precision mediump float;
uniform sampler2D u_y;
uniform sampler2D u_uv;
uniform mat3 u_matrix;
uniform vec3 u_offset;
in vec2 v_texCoord;
out vec4 fragColor;
void main() {
  float y = texture(u_y, v_texCoord).r;
  vec2 uv = texture(u_uv, v_texCoord).rg - vec2(0.5);
  vec3 rgb = u_matrix * vec3(y, uv.x, uv.y) + u_offset;
  fragColor = vec4(clamp(rgb, 0.0, 1.0), 1.0);
}`;

const POSITIONS = new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]);
const TEXCOORDS = new Float32Array([0, 1, 1, 1, 0, 0, 0, 0, 1, 1, 1, 0]);

export class WebGL2Renderer implements Renderer {
  readonly identity = 'webgl2';
  private surface: RendererSurface;
  private gl: WebGL2RenderingContext | null = null;
  private rgbaProgram: WebGLProgram | null = null;
  private yuvProgram: WebGLProgram | null = null;
  private nv12Program: WebGLProgram | null = null;
  private vao: WebGLVertexArrayObject | null = null;
  private positionBuffer: WebGLBuffer | null = null;
  private texCoordBuffer: WebGLBuffer | null = null;
  private yTexture: WebGLTexture | null = null;
  private uTexture: WebGLTexture | null = null;
  private vTexture: WebGLTexture | null = null;
  private uvTexture: WebGLTexture | null = null;
  private rgbaTexture: WebGLTexture | null = null;
  private metrics: MutableRendererMetrics = {
    framesSubmitted: 0,
    framesRendered: 0,
    framesDropped: 0,
    snapshotsTaken: 0,
    drawLatencyMs: 0,
  };

  constructor(canvas: HTMLCanvasElement | OffscreenCanvas) {
    this.surface = new RendererSurface(canvas);
  }

  async configure(config: RendererConfig): Promise<void> {
    this.surface.configure(config);
    this.gl = this.surface.getWebGlContext();
    if (!this.gl) {
      throw new RendererError('no-context', 'WebGL2 context not available');
    }
    this.compilePrograms();
    this.createBuffers();
    this.createTextures();
  }

  private compilePrograms(): void {
    const gl = this.gl;
    if (!gl) return;
    this.rgbaProgram = this.createProgram(gl, VERTEX_SHADER, FRAG_RGBA);
    this.yuvProgram = this.createProgram(gl, VERTEX_SHADER, FRAG_YUV);
    this.nv12Program = this.createProgram(gl, VERTEX_SHADER, FRAG_NV12);
  }

  private createProgram(gl: WebGL2RenderingContext, vs: string, fs: string): WebGLProgram {
    const vertex = gl.createShader(gl.VERTEX_SHADER);
    const fragment = gl.createShader(gl.FRAGMENT_SHADER);
    const program = gl.createProgram();
    if (!vertex || !fragment || !program) {
      throw new RendererError('shader-create', 'Failed to create WebGL shader objects');
    }
    gl.shaderSource(vertex, vs);
    gl.compileShader(vertex);
    gl.shaderSource(fragment, fs);
    gl.compileShader(fragment);
    gl.attachShader(program, vertex);
    gl.attachShader(program, fragment);
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      const info = gl.getProgramInfoLog(program) ?? 'unknown';
      throw new RendererError('shader-link', `WebGL program link failed: ${info}`);
    }
    return program;
  }

  private createBuffers(): void {
    const gl = this.gl;
    if (!gl) return;
    this.vao = gl.createVertexArray();
    gl.bindVertexArray(this.vao);

    this.positionBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.positionBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, POSITIONS, gl.STATIC_DRAW);

    this.texCoordBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.texCoordBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, TEXCOORDS, gl.STATIC_DRAW);

    gl.bindVertexArray(null);
  }

  private createTextures(): void {
    const gl = this.gl;
    if (!gl) return;
    this.yTexture = gl.createTexture();
    this.uTexture = gl.createTexture();
    this.vTexture = gl.createTexture();
    this.uvTexture = gl.createTexture();
    this.rgbaTexture = gl.createTexture();
  }

  async render(frame: RenderFrame): Promise<void> {
    const gl = this.gl;
    if (!gl) throw new RendererError('not-configured', 'WebGL2Renderer not configured');
    if (gl.isContextLost()) throw new RendererError('context-lost', 'WebGL2 context lost');

    const start = performance.now();
    const visibleRect = RendererSurface.resolveVisibleRect(frame);
    this.metrics.framesSubmitted += 1;
    try {
      const format = (frame.format ?? '').toLowerCase();
      const viewport = this.surface.computeViewport(visibleRect.width, visibleRect.height);

      gl.viewport(0, 0, this.surface.getCanvas().width, this.surface.getCanvas().height);
      gl.clearColor(0, 0, 0, 1);
      gl.clear(gl.COLOR_BUFFER_BIT);

      if (format === 'rgba') {
        await this.renderRgba(frame, visibleRect, viewport);
      } else if (format === 'i420') {
        await this.renderI420(frame, visibleRect, viewport);
      } else if (format === 'nv12') {
        await this.renderNv12(frame, visibleRect, viewport);
      } else {
        throw new RendererError('unsupported-format', `WebGL2Renderer does not support ${frame.format}`);
      }
      this.metrics.framesRendered += 1;
    } catch (err) {
      this.metrics.framesDropped += 1;
      throw err instanceof RendererError ? err : new RendererError('render-failed', String(err));
    } finally {
      this.metrics.drawLatencyMs = performance.now() - start;
    }
  }

  private async renderRgba(
    frame: RenderFrame,
    visibleRect: { x: number; y: number; width: number; height: number },
    viewport: { x: number; y: number; width: number; height: number },
  ): Promise<void> {
    const gl = this.gl;
    if (!gl) return;
    const size = frame.allocationSize({ rect: visibleRect });
    const data = new Uint8Array(size);
    await frame.copyTo(data, { rect: visibleRect });
    this.uploadTexture(this.rgbaTexture, data, visibleRect.width, visibleRect.height, gl.RGBA);
    this.drawQuad(this.rgbaProgram, [{ name: 'u_texture', texture: this.rgbaTexture, unit: 0 }], viewport);
  }

  private async renderI420(
    frame: RenderFrame,
    visibleRect: { x: number; y: number; width: number; height: number },
    viewport: { x: number; y: number; width: number; height: number },
  ): Promise<void> {
    const gl = this.gl;
    if (!gl) return;
    const cw = visibleRect.width;
    const ch = visibleRect.height;
    const halfW = Math.max(1, Math.floor(cw / 2));
    const halfH = Math.max(1, Math.floor(ch / 2));

    const ySize = cw * ch;
    const uSize = halfW * halfH;
    const yData = new Uint8Array(ySize);
    const uData = new Uint8Array(uSize);
    const vData = new Uint8Array(uSize);

    await frame.copyTo(yData, { planeIndex: 0, rect: visibleRect });
    await frame.copyTo(uData, { planeIndex: 1, rect: visibleRect });
    await frame.copyTo(vData, { planeIndex: 2, rect: visibleRect });

    this.uploadTexture(this.yTexture, yData, cw, ch, gl.RED);
    this.uploadTexture(this.uTexture, uData, halfW, halfH, gl.RED);
    this.uploadTexture(this.vTexture, vData, halfW, halfH, gl.RED);

    const { matrix, range } = resolveColorSpace(frame.colorSpace);
    const { coeffs, offset } = buildYuvToRgbCoeffs(matrix, range);
    this.drawQuad(
      this.yuvProgram,
      [
        { name: 'u_y', texture: this.yTexture, unit: 0 },
        { name: 'u_u', texture: this.uTexture, unit: 1 },
        { name: 'u_v', texture: this.vTexture, unit: 2 },
      ],
      viewport,
      coeffs,
      offset,
    );
  }

  private async renderNv12(
    frame: RenderFrame,
    visibleRect: { x: number; y: number; width: number; height: number },
    viewport: { x: number; y: number; width: number; height: number },
  ): Promise<void> {
    const gl = this.gl;
    if (!gl) return;
    const cw = visibleRect.width;
    const ch = visibleRect.height;
    const halfW = Math.max(1, Math.floor(cw / 2));
    const halfH = Math.max(1, Math.floor(ch / 2));
    const ySize = cw * ch;
    const uvSize = cw * halfH;
    const yData = new Uint8Array(ySize);
    const uvData = new Uint8Array(uvSize);
    await frame.copyTo(yData, { planeIndex: 0, rect: visibleRect });
    await frame.copyTo(uvData, { planeIndex: 1, rect: visibleRect });

    this.uploadTexture(this.yTexture, yData, cw, ch, gl.RED);
    this.uploadTexture(this.uvTexture, uvData, halfW, halfH, gl.RG);

    const { matrix, range } = resolveColorSpace(frame.colorSpace);
    const { coeffs, offset } = buildYuvToRgbCoeffs(matrix, range);
    this.drawQuad(
      this.nv12Program,
      [
        { name: 'u_y', texture: this.yTexture, unit: 0 },
        { name: 'u_uv', texture: this.uvTexture, unit: 1 },
      ],
      viewport,
      coeffs,
      offset,
    );
  }

  private uploadTexture(
    texture: WebGLTexture | null,
    data: Uint8Array,
    width: number,
    height: number,
    format: number,
  ): void {
    const gl = this.gl;
    if (!gl || !texture) return;
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    const internalFormat = format === gl.RED ? gl.R8 : format === gl.RG ? gl.RG8 : format === gl.RGBA ? gl.RGBA8 : format;
    const srcFormat = format;
    const srcType = gl.UNSIGNED_BYTE;
    gl.texImage2D(gl.TEXTURE_2D, 0, internalFormat, width, height, 0, srcFormat, srcType, data);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  }

  private drawQuad(
    program: WebGLProgram | null,
    samplers: { name: string; texture: WebGLTexture | null; unit: number }[],
    viewport: { x: number; y: number; width: number; height: number },
    coeffs?: number[],
    offset?: number[],
  ): void {
    const gl = this.gl;
    if (!gl || !program) return;
    gl.useProgram(program);
    gl.bindVertexArray(this.vao);

    const posLoc = gl.getAttribLocation(program, 'a_position');
    const texLoc = gl.getAttribLocation(program, 'a_texCoord');

    gl.bindBuffer(gl.ARRAY_BUFFER, this.positionBuffer);
    gl.enableVertexAttribArray(posLoc);
    gl.vertexAttribPointer(posLoc, 2, gl.FLOAT, false, 0, 0);

    gl.bindBuffer(gl.ARRAY_BUFFER, this.texCoordBuffer);
    gl.enableVertexAttribArray(texLoc);
    gl.vertexAttribPointer(texLoc, 2, gl.FLOAT, false, 0, 0);

    for (const { name, texture, unit } of samplers) {
      gl.activeTexture(gl.TEXTURE0 + unit);
      gl.bindTexture(gl.TEXTURE_2D, texture);
      const loc = gl.getUniformLocation(program, name);
      if (loc) gl.uniform1i(loc, unit);
    }

    if (coeffs && offset) {
      const matLoc = gl.getUniformLocation(program, 'u_matrix');
      const offLoc = gl.getUniformLocation(program, 'u_offset');
      if (matLoc) gl.uniformMatrix3fv(matLoc, false, new Float32Array(coeffs));
      if (offLoc) gl.uniform3fv(offLoc, new Float32Array(offset));
    }

    // Apply fit-mode viewport by scaling the clip-space quad.
    const canvasW = this.surface.getCanvas().width;
    const canvasH = this.surface.getCanvas().height;
    if (canvasW > 0 && canvasH > 0) {
      const transform = this.surface.getTransform();
      const angle = (transform.rotation * Math.PI) / 180;
      const cos = Math.cos(angle);
      const sin = Math.sin(angle);
      const mirror = transform.scaleX < 0 ? -1 : 1;
      const sx = (viewport.width / canvasW) * mirror;
      const sy = viewport.height / canvasH;
      const m00 = cos * sx;
      const m01 = -sin * sy;
      const m10 = sin * sx;
      const m11 = cos * sy;
      const tx = ((viewport.x + viewport.width / 2) / canvasW) * 2 - 1;
      const ty = 1 - ((viewport.y + viewport.height / 2) / canvasH) * 2;
      const matrix = new Float32Array([m00, m10, 0, m01, m11, 0, tx, ty, 1]);
      const matrixLoc = gl.getUniformLocation(program, 'u_viewMatrix');
      if (matrixLoc) gl.uniformMatrix3fv(matrixLoc, false, matrix);
    }

    gl.drawArrays(gl.TRIANGLES, 0, 6);
    gl.bindVertexArray(null);
  }

  async snapshot(opts: SnapshotOptions = {}): Promise<SnapshotResult> {
    const gl = this.gl;
    if (!gl) throw new RendererError('not-configured', 'WebGL2Renderer not configured');
    const options = validateSnapshotEncoderOptions(opts) as SnapshotOptions;

    const canvas = this.surface.getCanvas();
    const canvasW = canvas.width;
    const canvasH = canvas.height;
    let w = canvasW;
    let h = canvasH;
    if (options.maxWidth && options.maxHeight) {
      const scale = Math.min(1, options.maxWidth / canvasW, options.maxHeight / canvasH);
      w = Math.max(1, Math.floor(canvasW * scale));
      h = Math.max(1, Math.floor(canvasH * scale));
    }

    const raw = new Uint8Array(w * h * 4);

    if (w === canvasW && h === canvasH) {
      // Fast path: read directly from the default framebuffer.
      gl.bindFramebuffer(gl.FRAMEBUFFER, null);
      gl.readPixels(0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, raw);
    } else {
      // Blit from the default framebuffer into a scaled temp framebuffer so we
      // read a downscaled image instead of a crop.
      const { fb, tex } = this.createTempFramebuffer(w, h);
      gl.bindFramebuffer(gl.READ_FRAMEBUFFER, null);
      gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, fb);
      gl.blitFramebuffer(
        0, 0, canvasW, canvasH,
        0, 0, w, h,
        gl.COLOR_BUFFER_BIT, gl.LINEAR,
      );
      gl.bindFramebuffer(gl.FRAMEBUFFER, null);
      gl.flush();
      gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
      gl.readPixels(0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, raw);
      gl.bindFramebuffer(gl.FRAMEBUFFER, null);
      gl.deleteFramebuffer(fb);
      gl.deleteTexture(tex);
    }

    // WebGL readPixels is bottom-up; convert to top-down ImageData.
    const data = new Uint8ClampedArray(w * h * 4);
    for (let row = 0; row < h; row += 1) {
      const src = (h - 1 - row) * w * 4;
      const dst = row * w * 4;
      data.set(raw.subarray(src, src + w * 4), dst);
    }

    this.metrics.snapshotsTaken += 1;
    return { width: w, height: h, data: new ImageData(data, w, h) };
  }

  private createTempFramebuffer(width: number, height: number): { fb: WebGLFramebuffer; tex: WebGLTexture } {
    const gl = this.gl;
    if (!gl) throw new RendererError('no-context', 'WebGL2 context lost');
    const tex = gl.createTexture();
    const fb = gl.createFramebuffer();
    if (!tex || !fb) throw new RendererError('framebuffer-create', 'Failed to create WebGL framebuffer');
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, width, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    return { fb, tex };
  }

  getMetrics(): RendererMetrics {
    return { ...this.metrics };
  }

  close(): void {
    const gl = this.gl;
    if (gl) {
      gl.deleteProgram(this.rgbaProgram);
      gl.deleteProgram(this.yuvProgram);
      gl.deleteProgram(this.nv12Program);
      gl.deleteVertexArray(this.vao);
      [this.positionBuffer, this.texCoordBuffer].forEach((b) => b && gl.deleteBuffer(b));
      [this.yTexture, this.uTexture, this.vTexture, this.uvTexture, this.rgbaTexture].forEach(
        (t) => t && gl.deleteTexture(t),
      );
    }
    this.gl = null;
  }
}
