// Local verification probe for a separate, pinned Eurorack checkout.
// This original driver contains no Mutable Instruments DSP or resource data
// of its own; it only #includes the pinned upstream sources from the
// checkout named on the command line via the build script. It is never
// linked into Vactr's library, binary, or browser build.
//
// Generic across all 47 accessible Braids macro-oscillator shapes: the
// shape index, and both timbre/color parameters, are read from argv so one
// compiled executable can render every source position.

#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>

#include "braids/macro_oscillator.h"

namespace {
constexpr int kBlock = 24;
constexpr int kBlocks = 200;
// 60 << 7: the pinned firmware's own default quantizer target
// ("(60 + settings.quantizer_root()) << 7" in braids.cc, root 0), i.e.
// MIDI note 60 in the source's 128-units-per-semitone pitch code. This is
// not an inferred constant; it is the literal expression the reference
// firmware uses for its default note.
constexpr int16_t kPitchCode = 60 << 7;

braids::MacroOscillator osc;

bool unit(const char* text, float* value) {
  char* end = nullptr;
  const float parsed = std::strtof(text, &end);
  if (end == text || *end != '\0' || !std::isfinite(parsed) ||
      parsed < 0.0f || parsed > 1.0f) {
    return false;
  }
  *value = parsed;
  return true;
}

bool shape_index(const char* text, long* value) {
  char* end = nullptr;
  const long parsed = std::strtol(text, &end, 10);
  if (end == text || *end != '\0' || parsed < 0 || parsed > 46) {
    return false;
  }
  *value = parsed;
  return true;
}
}  // namespace

int main(int argc, char** argv) {
  long shape = 0;
  float timbre = 0.5f;
  float color = 0.5f;
  if (argc != 4 || !shape_index(argv[1], &shape) || !unit(argv[2], &timbre) ||
      !unit(argv[3], &color)) {
    std::fputs("expected shape(0..46) timbre(0..1) color(0..1)\n", stderr);
    return 1;
  }

  osc.Init();
  osc.set_shape(static_cast<braids::MacroOscillatorShape>(shape));
  osc.set_pitch(kPitchCode);
  const int16_t timbre_code =
      static_cast<int16_t>(std::lround(timbre * 32767.0f));
  const int16_t color_code =
      static_cast<int16_t>(std::lround(color * 32767.0f));
  osc.set_parameters(timbre_code, color_code);
  osc.Strike();

  uint8_t sync_buffer[kBlock];
  for (int frame = 0; frame < kBlock; ++frame) {
    sync_buffer[frame] = 0;
  }

  for (int block = 0; block < kBlocks; ++block) {
    int16_t render_buffer[kBlock];
    osc.Render(sync_buffer, render_buffer, kBlock);
    for (int frame = 0; frame < kBlock; ++frame) {
      const float sample = render_buffer[frame] / 32768.0f;
      if (!std::isfinite(sample)) {
        std::fputs("nonfinite upstream Braids output\n", stderr);
        return 2;
      }
      if (std::printf("%.9g\n", sample) < 0) {
        return 3;
      }
    }
  }
  return std::ferror(stdout) ? 3 : 0;
}
