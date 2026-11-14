import type { CodeRect } from '../app/apis';

export const INSTANCE_BYTES = 32;
export const INSTANCES_PER_BLOCK = 64;
export const MAX_TEXT_BLOCKS = 1536;
export const CLIP_GUTTER = 1;
export const UNTINTED = 2;
export const TEXT_SPACE = 4;
export const NO_HSCROLL = 8;
export type GeometryLayer = 'background' | 'text' | 'overlay' | 'overlayText';
export type InstanceRect = CodeRect | readonly [number, number, number, number];
export type InstanceUv = readonly [number, number, number, number];
export type InstanceColor = readonly [number, number, number, number];

export const VERTEX_SOURCE = `#version 300 es
in vec2 a_corner;
in vec4 a_rect;
in vec4 a_uv;
in vec4 a_color;
in uvec2 a_meta;
uniform vec2 u_view;
uniform sampler2D u_slots;
out vec2 v_uv;
out vec4 v_color;
out float v_screen_x;
flat out uint v_flags;
void main() {
  vec2 slot = texelFetch(u_slots, ivec2(int(a_meta.x), 0), 0).rg;
  vec2 p = a_rect.xy + slot + a_corner * a_rect.zw;
  v_screen_x = p.x;
  v_uv = mix(a_uv.xy, a_uv.zw, a_corner);
  v_color = a_color;
  v_flags = a_meta.y;
  gl_Position = vec4(p.x / u_view.x * 2.0 - 1.0, 1.0 - p.y / u_view.y * 2.0, 0.0, 1.0);
}`;

export const FRAGMENT_SOURCE = `#version 300 es
precision mediump float;
precision highp int;
in vec2 v_uv;
in vec4 v_color;
in float v_screen_x;
flat in uint v_flags;
uniform sampler2D u_texture;
uniform float u_gutter;
out vec4 out_color;
void main() {
  if ((v_flags & 1u) != 0u && v_screen_x < u_gutter) discard;
  vec4 texel = texture(u_texture, v_uv);
  out_color = (v_flags & 2u) != 0u ? texel : texel * v_color;
}`;

/** Ledger-owned growable GPU instance buffer for one compositing layer. */
export class LayerBuffer {
  private reservation: import('./resources').Reservation | null = null;
  private capacity = 0;
  constructor(private gl: WebGL2RenderingContext, private ledger: import('./resources').ResourceLedger,
    readonly buffer: WebGLBuffer, private maximumBytes = Number.MAX_SAFE_INTEGER) {}
  get capacityBytes(): number { return this.capacity; }
  upload(bytes: Uint8Array): void {
    this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.buffer);
    this.ensureCapacity(bytes.byteLength);
    if (bytes.byteLength) this.gl.bufferSubData(this.gl.ARRAY_BUFFER, 0, bytes);
  }
  uploadRange(offset: number, bytes: Uint8Array): void {
    if (!Number.isSafeInteger(offset) || offset < 0 || offset % INSTANCE_BYTES !== 0) throw new RangeError('Invalid geometry range');
    this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.buffer);
    this.ensureCapacity(offset + bytes.byteLength);
    if (bytes.byteLength) this.gl.bufferSubData(this.gl.ARRAY_BUFFER, offset, bytes);
  }
  private ensureCapacity(required: number): void {
    if (required <= this.capacity) return;
    if (required > this.maximumBytes) throw new Error('Layer geometry block cap reached');
    let next = Math.max(INSTANCE_BYTES * INSTANCES_PER_BLOCK, this.capacity || 1);
    while (next < required) next = Math.min(this.maximumBytes, next * 2);
    const reservation = this.ledger.allocate('geometry', next);
    if (!reservation) throw new Error('Layer geometry budget exhausted');
    try {
      this.gl.bufferData(this.gl.ARRAY_BUFFER, next, this.gl.DYNAMIC_DRAW);
      if (this.gl.getError() !== this.gl.NO_ERROR) throw new Error('Layer allocation failed');
    }
    catch (error) { reservation.release(); throw error; }
    this.reservation?.release(); this.reservation = reservation; this.capacity = next;
  }
  dispose(): void { this.reservation?.release(); this.reservation = null; this.capacity = 0; }
}

/** Reusable little-endian writer for the stable 32-byte WebGL instance format. */
export class InstanceWriter {
  private storage: ArrayBuffer = new ArrayBuffer(INSTANCE_BYTES * INSTANCES_PER_BLOCK);
  private view = new DataView(this.storage);
  private length = 0;
  clear(): void { new Uint8Array(this.storage, 0, this.length * INSTANCE_BYTES).fill(0); this.length = 0; }
  get count(): number { return this.length; }
  bytes(): Uint8Array { return new Uint8Array(this.storage, 0, this.length * INSTANCE_BYTES); }
  push(rect: InstanceRect, uv: InstanceUv, color: InstanceColor, slot = 0, flags = 0): void {
    this.pushAt(this.length, rect, uv, color, slot, flags);
  }
  pushAt(index: number, rect: InstanceRect, uv: InstanceUv, color: InstanceColor, slot = 0, flags = 0): void {
    if (!Number.isSafeInteger(index) || index < 0) throw new RangeError('Invalid instance index');
    const offset = index * INSTANCE_BYTES;
    this.ensure(offset + INSTANCE_BYTES);
    if ('left' in rect) {
      this.view.setFloat32(offset, rect.left, true); this.view.setFloat32(offset + 4, rect.top, true);
      this.view.setFloat32(offset + 8, rect.right - rect.left, true); this.view.setFloat32(offset + 12, rect.bottom - rect.top, true);
    } else {
      for (let index = 0; index < 4; index++) this.view.setFloat32(offset + index * 4, rect[index]!, true);
    }
    for (let index = 0; index < 4; index++) this.view.setUint16(offset + 16 + index * 2, quantize(uv[index]!, 65535), true);
    for (let index = 0; index < 4; index++) this.view.setUint8(offset + 24 + index, quantize(color[index]!, 255));
    this.view.setUint16(offset + 28, slot, true);
    this.view.setUint16(offset + 30, flags, true);
    this.length = Math.max(this.length, index + 1);
  }
  setCount(count: number): void {
    if (!Number.isSafeInteger(count) || count < 0) throw new RangeError('Invalid instance count');
    this.ensure(count * INSTANCE_BYTES); this.length = count;
  }
  padToBlock(): void {
    const aligned = Math.ceil(this.length / INSTANCES_PER_BLOCK) * INSTANCES_PER_BLOCK;
    if (aligned === this.length) return;
    this.ensure(aligned * INSTANCE_BYTES);
    new Uint8Array(this.storage, this.length * INSTANCE_BYTES, (aligned - this.length) * INSTANCE_BYTES).fill(0);
    this.length = aligned;
  }
  private ensure(required: number): void {
    if (required <= this.storage.byteLength) return;
    let capacity = this.storage.byteLength;
    while (capacity < required) capacity *= 2;
    const storage = new ArrayBuffer(capacity);
    new Uint8Array(storage).set(new Uint8Array(this.storage, 0, this.length * INSTANCE_BYTES));
    this.storage = storage;
    this.view = new DataView(storage);
  }
}

function quantize(value: number, max: number): number {
  return Math.max(0, Math.min(max, Math.round(value * max)));
}
