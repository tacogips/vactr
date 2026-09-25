// `TextAsset` rasterization (design 9.2, 15.1.8): the `text "hello"`
// source's payload is drawn with canvas 2D (fixed font and size, white on
// transparent) and uploaded to a texture that the render host binds at the
// `u_text<id>` sampler. The asset is plain text; nothing is evaluated.

export const TEXT_ASSET_WIDTH = 512;
export const TEXT_ASSET_HEIGHT = 128;
export const TEXT_ASSET_FONT = 'bold 64px monospace';

export interface TextAsset {
  id: number;
  text: string;
}

export type CanvasFactory = () => HTMLCanvasElement;

export type TextAssetResult = { ok: true; texture: WebGLTexture } | { ok: false; message: string };

/** Draws `text` centered into a fresh canvas; null when there is no 2D context. */
export function rasterizeText(createCanvas: CanvasFactory, text: string): HTMLCanvasElement | null {
  const canvas = createCanvas();
  canvas.width = TEXT_ASSET_WIDTH;
  canvas.height = TEXT_ASSET_HEIGHT;
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;
  ctx.clearRect(0, 0, TEXT_ASSET_WIDTH, TEXT_ASSET_HEIGHT);
  ctx.font = TEXT_ASSET_FONT;
  ctx.fillStyle = '#ffffff';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(text, TEXT_ASSET_WIDTH / 2, TEXT_ASSET_HEIGHT / 2, TEXT_ASSET_WIDTH - 16);
  return canvas;
}

/** Rasterizes `asset` and uploads it to a new texture. */
export function uploadTextAsset(
  gl: WebGL2RenderingContext,
  createCanvas: CanvasFactory,
  asset: TextAsset,
): TextAssetResult {
  const canvas = rasterizeText(createCanvas, asset.text);
  if (!canvas) return { ok: false, message: `2D canvas not available for text asset ${asset.id}` };
  const texture = gl.createTexture();
  if (!texture) return { ok: false, message: `no texture for text asset ${asset.id}` };
  gl.bindTexture(gl.TEXTURE_2D, texture);
  // Canvas rows run top-down; `gl_FragCoord` runs bottom-up.
  gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, canvas);
  gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.bindTexture(gl.TEXTURE_2D, null);
  return { ok: true, texture };
}
