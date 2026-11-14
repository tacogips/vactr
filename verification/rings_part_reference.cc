// Local verification probe for a separate, pinned Eurorack checkout.
// This original driver contains no Mutable Instruments DSP or resource data.
// It is never linked into Vactr's library, binary, or browser build.
//
// Drives the pinned upstream rings::Part (six ResonatorModel values) and
// rings::StringSynthPart directly at the source rate/block (48 kHz, 24
// samples per rings::dsp::dsp.h) with polyphony fixed at 1, an internal
// strum/exciter trigger on the first block, and a fixed note (69, i.e. A4 /
// 440 Hz with tonic and fm at zero). No external excitation is applied.

#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>

#include "rings/dsp/part.h"
#include "rings/dsp/string_synth_part.h"

namespace {

constexpr int kBlock = 24;               // rings::kMaxBlockSize
constexpr int kBlocks = 4000;             // 2.0 s at 48 kHz
constexpr float kFixedNote = 69.0f;       // A4; frequency = a3 with tonic/fm 0

// rings::Reverb (and StringSynthPart's shared chorus/ensemble/reverb) use
// FxEngine<32768, FORMAT_16_BIT>; static storage keeps this off the stack.
uint16_t reverb_buffer[32768];

rings::Part part;
rings::StringSynthPart string_synth_part;

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

void emit(const float* out, const float* aux, int size) {
  for (int frame = 0; frame < size; ++frame) {
    if (!std::isfinite(out[frame]) || !std::isfinite(aux[frame])) {
      std::fputs("nonfinite upstream Rings output\n", stderr);
      std::exit(2);
    }
    if (std::printf("%.9g %.9g\n", out[frame], aux[frame]) < 0) {
      std::exit(3);
    }
  }
}

}  // namespace

int main(int argc, char** argv) {
  if (argc != 6) {
    std::fputs(
        "expected: target structure brightness damping position\n"
        "target is 0..5 (rings::ResonatorModel ordinal) or \"stringsynth\"\n",
        stderr);
    return 1;
  }

  rings::Patch patch{};
  if (!control(argv[2], &patch.structure) ||
      !control(argv[3], &patch.brightness) ||
      !control(argv[4], &patch.damping) ||
      !control(argv[5], &patch.position)) {
    std::fputs("structure/brightness/damping/position must be 0..1\n",
               stderr);
    return 1;
  }

  const float in[kBlock] = {0.0f};  // No external excitation: internal only.

  rings::PerformanceState performance_state{};
  performance_state.internal_exciter = true;
  performance_state.internal_strum = true;
  performance_state.internal_note = true;
  performance_state.tonic = 0.0f;
  performance_state.note = kFixedNote;
  performance_state.fm = 0.0f;
  performance_state.chord = 0;

  char* end = nullptr;
  const long model_index = std::strtol(argv[1], &end, 10);
  const bool is_model = *argv[1] != '\0' && *end == '\0' && model_index >= 0 &&
                         model_index < rings::RESONATOR_MODEL_LAST;

  if (is_model) {
    part.Init(reverb_buffer);
    part.set_polyphony(1);
    part.set_model(static_cast<rings::ResonatorModel>(model_index));
    for (int block = 0; block < kBlocks; ++block) {
      float out[kBlock];
      float aux[kBlock];
      performance_state.strum = (block == 0);
      part.Process(performance_state, patch, in, out, aux, kBlock);
      emit(out, aux, kBlock);
    }
    return std::ferror(stdout) ? 3 : 0;
  }

  if (std::strcmp(argv[1], "stringsynth") == 0) {
    string_synth_part.Init(reverb_buffer);
    string_synth_part.set_polyphony(1);
    for (int block = 0; block < kBlocks; ++block) {
      float out[kBlock];
      float aux[kBlock];
      performance_state.strum = (block == 0);
      string_synth_part.Process(performance_state, patch, in, out, aux,
                                 kBlock);
      emit(out, aux, kBlock);
    }
    return std::ferror(stdout) ? 3 : 0;
  }

  std::fputs("unknown target: expected 0..5 or \"stringsynth\"\n", stderr);
  return 1;
}
