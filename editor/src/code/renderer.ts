import { CanvasRenderer as EmakiCanvasRenderer, type RendererOptions, type TextLayout } from 'emaki/engine';
import { toRenderInput, type RenderFeedback } from './decorations';
import { FALLBACK_PALETTE, type VactrPalette } from './theme';
export { GPU_TOKEN_COLORS } from './theme';
export type { GpuStatus } from 'emaki/engine';
export type { RenderFeedback } from './decorations';

export class CanvasRenderer extends EmakiCanvasRenderer {
  private vactrPalette: VactrPalette;
  constructor(canvas: HTMLCanvasElement, layout: TextLayout, options: Omit<RendererOptions, 'palette'> & { palette?: VactrPalette } = {}) {
    const palette = options.palette ?? FALLBACK_PALETTE;
    super(canvas, layout, { ...options, palette });
    this.vactrPalette = palette;
  }
  setPalette(palette: VactrPalette): void {
    this.vactrPalette = palette as VactrPalette;
    super.setPalette(palette);
  }
  render(feedback: RenderFeedback = {}): boolean {
    return super.render(toRenderInput(feedback as RenderFeedback, this.vactrPalette));
  }
}
