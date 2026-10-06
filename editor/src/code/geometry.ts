import type { CodeRect } from '../app/apis';

export const INSTANCE_BYTES = 32;
export const INSTANCES_PER_BLOCK = 64;
export const MAX_TEXT_BLOCKS = 1536;
export type GeometryLayer = 'background' | 'text' | 'overlay' | 'overlayText';

export interface GeometryInstance {
  rect: CodeRect;
  uv: readonly [number, number, number, number];
  color: readonly [number, number, number, number];
}

/** Pack one instanced quad using the renderer's stable 32-byte GPU layout. */
export function packInstances(instances: readonly GeometryInstance[]): ArrayBuffer {
  const bytes = new ArrayBuffer(instances.length * INSTANCE_BYTES);
  const view = new DataView(bytes);
  instances.forEach((instance, index) => {
    const offset = index * INSTANCE_BYTES;
    const { rect, uv, color } = instance;
    [rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top].forEach((value, part) => view.setFloat32(offset + part * 4, value, true));
    uv.forEach((value, part) => view.setUint16(offset + 16 + part * 2, Math.max(0, Math.min(65535, Math.round(value * 65535))), true));
    color.forEach((value, part) => view.setUint8(offset + 24 + part, Math.max(0, Math.min(255, Math.round(value * 255)))));
  });
  return bytes;
}
