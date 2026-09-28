// Local numerical probe for the separately checked-out, MIT-noticed
// Mutable Instruments Warps vocoder. No upstream DSP or assets live here.

#include <cmath>
#include <cstdio>
#include <cstdlib>

#include "warps/dsp/vocoder.h"

int main(int argc, char** argv) {
  if (argc != 5) {
    std::fprintf(stderr,
                 "usage: warps_vocoder_reference RELEASE FORMANT GATE_BLOCK AFTER_RELEASE\n");
    return 2;
  }
  const float release = std::strtof(argv[1], nullptr);
  const float formant = std::strtof(argv[2], nullptr);
  const int gate_block = std::atoi(argv[3]);
  const float after_release = std::strtof(argv[4], nullptr);
  if (!std::isfinite(release) || release < 0.0f || release > 1.0f ||
      !std::isfinite(formant) || formant < 0.0f || formant > 1.0f ||
      !std::isfinite(after_release) || after_release < 0.0f ||
      after_release > 1.0f ||
      gate_block < 0 || gate_block > 100) {
    std::fprintf(stderr, "controls must be finite in 0..1; gate 0..100\n");
    return 2;
  }

  constexpr int kBlock = 60;
  constexpr int kBlocks = 100;
  constexpr float kRate = 96000.0f;
  constexpr float kTau = 6.2831853071795864769f;
  warps::Vocoder vocoder;
  vocoder.Init(kRate);
  vocoder.set_release_time(release);
  vocoder.set_formant_shift(formant);
  float modulator[kBlock];
  float carrier[kBlock];
  float output[kBlock];
  for (int block = 0; block < kBlocks; ++block) {
    if (block == gate_block) {
      vocoder.set_release_time(after_release);
    }
    for (int i = 0; i < kBlock; ++i) {
      const float time = static_cast<float>(block * kBlock + i) / kRate;
      modulator[i] = 0.65f * std::sin(kTau * 220.0f * time) +
                     0.20f * std::sin(kTau * 330.0f * time);
      if (block >= gate_block) {
        modulator[i] = 0.0f;
      }
      carrier[i] = 0.55f * std::sin(kTau * 110.0f * time) +
                   0.25f * std::sin(kTau * 880.0f * time);
    }
    vocoder.Process(modulator, carrier, output, kBlock);
    for (float sample : output) {
      if (!std::isfinite(sample)) {
        std::fprintf(stderr, "source emitted non-finite audio\n");
        return 1;
      }
      std::printf("%.9g\n", sample);
    }
  }
  return 0;
}
