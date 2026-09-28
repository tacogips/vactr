// Local verification probe for a separate, pinned Eurorack checkout.
// This original driver contains no Mutable Instruments DSP or resource data.
// It is never linked into Vactr's library, binary, or browser build.

#include <cmath>
#include <cstdio>
#include <cstdlib>

#include "plaits/dsp/engine/fm_engine.h"

namespace {
constexpr int kBlock = 24;
constexpr int kBlocks = 100;

// Static storage zeroes the source FM engine's FIR accumulators before Init.
// The pinned Init routine does not initialize those two fields.
plaits::FMEngine engine;

bool control(const char* text, float* value) {
  char* end = nullptr;
  const float parsed = std::strtof(text, &end);
  if (end == text || *end != '\0' || !std::isfinite(parsed) ||
      parsed < 0.0f || parsed > 1.0f) {
    return false;
  }
  *value = parsed;
  return true;
}
}  // namespace

int main(int argc, char** argv) {
  float harmonics = 0.5f;
  float timbre = 0.5f;
  float morph = 0.5f;
  if (argc != 1 && (argc != 4 || !control(argv[1], &harmonics) ||
                    !control(argv[2], &timbre) ||
                    !control(argv[3], &morph))) {
    std::fputs("expected harmonics timbre morph, each 0..1\n", stderr);
    return 1;
  }
  engine.Init(nullptr);

  plaits::EngineParameters parameters{};
  parameters.note = 69.0f;
  parameters.harmonics = harmonics;
  parameters.timbre = timbre;
  parameters.morph = morph;
  parameters.trigger = plaits::TRIGGER_LOW;

  for (int block = 0; block < kBlocks; ++block) {
    float main[kBlock];
    float aux[kBlock];
    bool already_enveloped = false;
    engine.Render(parameters, main, aux, kBlock, &already_enveloped);
    for (int frame = 0; frame < kBlock; ++frame) {
      if (!std::isfinite(main[frame]) || !std::isfinite(aux[frame])) {
        std::fputs("nonfinite upstream FM output\n", stderr);
        return 2;
      }
      if (std::printf("%.9g %.9g\n", main[frame], aux[frame]) < 0) {
        return 3;
      }
    }
  }
  return std::ferror(stdout) ? 3 : 0;
}
