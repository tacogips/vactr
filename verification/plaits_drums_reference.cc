// Local verification probe for a separate, pinned Eurorack checkout.
// This original driver contains no Mutable Instruments DSP or resource data.
// It is never linked into Vactr's library, binary, or browser build.

#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>

#include "plaits/dsp/engine/bass_drum_engine.h"
#include "plaits/dsp/engine/hi_hat_engine.h"
#include "plaits/dsp/engine/snare_drum_engine.h"

namespace {
constexpr int kBlock = 24;
constexpr int kBlocks = 3000;

// Fixed per-engine notes fed to the pinned source's own NoteToFrequency;
// the matching Vactr-side Hz value is documented and computed in
// examples/plaits_drums_reference.rs.
constexpr float kKickNote = 36.0f;
constexpr float kSnareNote = 57.0f;
constexpr float kHatNote = 81.0f;

// Static storage zeroes engine state before Init, as plaits_fm_reference.cc
// does for the FM engine.
plaits::BassDrumEngine kick_engine;
plaits::SnareDrumEngine snare_engine;
plaits::HiHatEngine hat_engine;
uint8_t hat_allocator_buffer[4096];

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
  if (argc != 6) {
    std::fputs(
        "expected: engine(kick|snare|hihat) harmonics timbre morph accent\n",
        stderr);
    return 1;
  }
  float harmonics = 0.0f;
  float timbre = 0.0f;
  float morph = 0.0f;
  float accent = 0.0f;
  if (!control(argv[2], &harmonics) || !control(argv[3], &timbre) ||
      !control(argv[4], &morph) || !control(argv[5], &accent)) {
    std::fputs("controls must each be finite and within 0..1\n", stderr);
    return 1;
  }

  plaits::Engine* engine = nullptr;
  plaits::EngineParameters parameters{};
  parameters.harmonics = harmonics;
  parameters.timbre = timbre;
  parameters.morph = morph;
  parameters.accent = accent;

  const char* engine_name = argv[1];
  if (std::strcmp(engine_name, "kick") == 0) {
    kick_engine.Init(nullptr);
    parameters.note = kKickNote;
    engine = &kick_engine;
  } else if (std::strcmp(engine_name, "snare") == 0) {
    snare_engine.Init(nullptr);
    parameters.note = kSnareNote;
    engine = &snare_engine;
  } else if (std::strcmp(engine_name, "hihat") == 0) {
    stmlib::BufferAllocator allocator(hat_allocator_buffer,
                                       sizeof(hat_allocator_buffer));
    hat_engine.Init(&allocator);
    parameters.note = kHatNote;
    engine = &hat_engine;
  } else {
    std::fputs("engine must be kick, snare or hihat\n", stderr);
    return 1;
  }

  for (int block = 0; block < kBlocks; ++block) {
    // A single rising-edge trigger on the very first block, then TRIGGER_LOW:
    // one retrigger for the whole run, matching the Vactr probe's freshly
    // reset NodeState.
    parameters.trigger =
        block == 0 ? plaits::TRIGGER_RISING_EDGE : plaits::TRIGGER_LOW;
    float main_out[kBlock];
    float aux_out[kBlock];
    bool already_enveloped = false;
    engine->Render(parameters, main_out, aux_out, kBlock, &already_enveloped);
    for (int frame = 0; frame < kBlock; ++frame) {
      if (!std::isfinite(main_out[frame]) || !std::isfinite(aux_out[frame])) {
        std::fputs("nonfinite upstream drum output\n", stderr);
        return 2;
      }
      if (std::printf("%.9g %.9g\n", main_out[frame], aux_out[frame]) < 0) {
        return 3;
      }
    }
  }
  return std::ferror(stdout) ? 3 : 0;
}
