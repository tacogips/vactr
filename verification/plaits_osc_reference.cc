// Local-only driver for raw Plaits oscillator-family engines.
// This file is original harness code. Upstream source/resources are linked
// from a separate pinned checkout into a temporary executable only.

#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>

#include "plaits/dsp/engine/additive_engine.h"
#include "plaits/dsp/engine/grain_engine.h"
#include "plaits/dsp/engine/virtual_analog_engine.h"
#include "plaits/dsp/engine/waveshaping_engine.h"
#include "plaits/dsp/engine2/chiptune_engine.h"
#include "plaits/dsp/engine2/phase_distortion_engine.h"
#include "plaits/dsp/engine2/virtual_analog_vcf_engine.h"

namespace {
constexpr size_t kBlock = 24;
constexpr size_t kBlocks = 3000;
constexpr float kNote = 69.0f;
alignas(16) uint8_t allocator_memory[65536];
plaits::VirtualAnalogVCFEngine position_0;
plaits::PhaseDistortionEngine position_1;
plaits::ChiptuneEngine position_7;
plaits::VirtualAnalogEngine position_8;
plaits::WaveshapingEngine position_9;
plaits::GrainEngine position_11;
plaits::AdditiveEngine position_12;

bool number(const char* text, float* value) {
  char* end = nullptr;
  const float parsed = std::strtof(text, &end);
  if (end == text || *end != '\0' || !std::isfinite(parsed)) return false;
  *value = parsed;
  return true;
}
}  // namespace

int main(int argc, char** argv) {
  if (argc != 7) {
    std::fputs("expected: position harmonics timbre morph accent trigger\n", stderr);
    return 1;
  }
  char* end = nullptr;
  const long position = std::strtol(argv[1], &end, 10);
  if (end == argv[1] || *end != '\0') return 1;
  float harmonics = 0.0f, timbre = 0.0f, morph = 0.0f, accent = 0.0f, trigger = 0.0f;
  if (!number(argv[2], &harmonics) || !number(argv[3], &timbre) ||
      !number(argv[4], &morph) || !number(argv[5], &accent) ||
      !number(argv[6], &trigger) || harmonics < 0.0f || harmonics > 1.0f ||
      timbre < 0.0f || timbre > 1.0f || morph < 0.0f || morph > 1.0f ||
      accent < 0.0f || accent > 1.0f || trigger < 0.0f || trigger > 1.0f) {
    std::fputs("controls must be finite and within 0..1\n", stderr);
    return 1;
  }

  stmlib::BufferAllocator allocator(allocator_memory, sizeof(allocator_memory));
  plaits::Engine* engine = nullptr;
  switch (position) {
    case 0: engine = &position_0; break;
    case 1: engine = &position_1; break;
    case 7: engine = &position_7; break;
    case 8: engine = &position_8; break;
    case 9: engine = &position_9; break;
    case 11: engine = &position_11; break;
    case 12: engine = &position_12; break;
    default: std::fputs("position must be one of 0,1,7,8,9,11,12\n", stderr); return 1;
  }
  engine->Init(&allocator);

  plaits::EngineParameters parameters{};
  parameters.note = kNote;
  parameters.harmonics = harmonics;
  parameters.timbre = timbre;
  parameters.morph = morph;
  parameters.accent = accent;
  for (size_t block = 0; block < kBlocks; ++block) {
    // One source trigger edge on block zero, then release. Oscillator kernels
    // are otherwise continuous; the trigger field remains part of the input.
    parameters.trigger = trigger >= 0.5f
        ? (block == 0 ? plaits::TRIGGER_RISING_EDGE : plaits::TRIGGER_LOW)
        : plaits::TRIGGER_UNPATCHED;
    float main[kBlock], aux[kBlock];
    bool already_enveloped = false;
    engine->Render(parameters, main, aux, kBlock, &already_enveloped);
    for (size_t i = 0; i < kBlock; ++i) {
      if (!std::isfinite(main[i]) || !std::isfinite(aux[i])) {
        std::fputs("nonfinite upstream output\n", stderr);
        return 2;
      }
      if (std::printf("%.9g %.9g\n", main[i], aux[i]) < 0) return 3;
    }
  }
  return std::ferror(stdout) ? 3 : 0;
}
