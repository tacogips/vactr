// Local verification probe for a separate, pinned Eurorack checkout.
// This original driver contains no Mutable Instruments DSP or resource data.
// It is never linked into Vactr's library, binary, or browser build.

#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>

#include "elements/dsp/part.h"

namespace {
constexpr size_t kBlock = 16;
constexpr size_t kBlocks = 6000;
constexpr size_t kFrames = kBlock * kBlocks;
constexpr size_t kReverbWords = 32768;
constexpr float kNote = 60.0f;

elements::Part part;
uint16_t reverb_buffer[kReverbWords];

bool parse_control(const char* text, float* value) {
  char* end = nullptr;
  const float parsed = std::strtof(text, &end);
  if (end == text || *end != '\0' || !std::isfinite(parsed)) {
    return false;
  }
  *value = parsed;
  return true;
}

bool set_model(const char* text) {
  if (std::strcmp(text, "modal") == 0) {
    part.set_resonator_model(elements::RESONATOR_MODEL_MODAL);
  } else if (std::strcmp(text, "string") == 0) {
    part.set_resonator_model(elements::RESONATOR_MODEL_STRING);
  } else if (std::strcmp(text, "strings") == 0) {
    part.set_resonator_model(elements::RESONATOR_MODEL_STRINGS);
  } else {
    return false;
  }
  return true;
}
}  // namespace

int main(int argc, char** argv) {
  // model followed by the twenty ex-* patch fields in exciter-core order.
  if (argc != 22) {
    std::fputs("expected: model and 20 finite ex-* patch controls\n", stderr);
    return 1;
  }
  if (!set_model(argv[1])) {
    std::fputs("model must be modal, string or strings\n", stderr);
    return 1;
  }

  part.Init(reverb_buffer);
  if (!set_model(argv[1])) {
    return 1;
  }
  elements::Patch* patch = part.mutable_patch();
  float* controls[] = {
      &patch->exciter_envelope_shape,
      &patch->exciter_bow_level,
      &patch->exciter_bow_timbre,
      &patch->exciter_blow_level,
      &patch->exciter_blow_meta,
      &patch->exciter_blow_timbre,
      &patch->exciter_strike_level,
      &patch->exciter_strike_meta,
      &patch->exciter_strike_timbre,
      &patch->exciter_signature,
      &patch->resonator_geometry,
      &patch->resonator_brightness,
      &patch->resonator_damping,
      &patch->resonator_position,
      &patch->resonator_modulation_frequency,
      &patch->resonator_modulation_offset,
      &patch->reverb_diffusion,
      &patch->reverb_lp,
      &patch->space,
      &patch->modulation_frequency,
  };
  for (size_t index = 0; index < 20; ++index) {
    if (!parse_control(argv[index + 2], controls[index])) {
      std::fputs("each ex-* patch control must be finite\n", stderr);
      return 1;
    }
  }

  elements::PerformanceState performance{};
  performance.gate = true;
  performance.note = kNote;
  performance.modulation = 0.0f;
  performance.strength = 1.0f;

  float blow[kBlock]{};
  float strike[kBlock]{};
  float main[kBlock]{};
  float aux[kBlock]{};
  for (size_t block = 0; block < kBlocks; ++block) {
    part.Process(performance, blow, strike, main, aux, kBlock);
    for (size_t frame = 0; frame < kBlock; ++frame) {
      if (!std::isfinite(main[frame]) || !std::isfinite(aux[frame])) {
        std::fputs("nonfinite upstream Elements output\n", stderr);
        return 2;
      }
      if (std::printf("%.9g %.9g\n", main[frame], aux[frame]) < 0) {
        return 3;
      }
    }
  }
  return std::ferror(stdout) ? 3 : 0;
}
