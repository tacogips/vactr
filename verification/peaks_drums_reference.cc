// Local verification probe for a separate, pinned Eurorack checkout.
// This original driver contains no Mutable Instruments DSP or resource data.
// It is never linked into Vactr's library, binary, or browser build.
//
// Drives one of the four pinned Peaks drum voices (peaks/drums/{bass_drum,
// snare_drum,high_hat,fm_drum}.{h,cc}) for a single triggered hit at the
// firmware's own 48 kHz rate and prints normalized float samples, one per
// line, for verification/compare_peaks_drums.py to compare against Vactr's
// analog-percussion and fm-drum kernels.

#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>

#include "stmlib/utils/random.h"

#include "peaks/drums/bass_drum.h"
#include "peaks/drums/fm_drum.h"
#include "peaks/drums/high_hat.h"
#include "peaks/drums/snare_drum.h"

namespace {

// The firmware's own rate; peaks/drums/{bass_drum,snare_drum}.cc hard-code
// 1.0e-3 * 48000 and 4.0e-3 * 48000 excitation delays.
constexpr int kSampleRate = 48000;
constexpr double kDurationSeconds = 2.5;
constexpr int kFrames = static_cast<int>(kSampleRate * kDurationSeconds);

// Static storage zero-initializes every field Init() does not touch itself,
// matching the firmware's global voice instances (notably HighHat::phase_).
peaks::BassDrum bass_drum;
peaks::SnareDrum snare_drum;
peaks::HighHat high_hat;
peaks::FmDrum fm_drum;

bool parse_unit(const char* text, uint16_t* value) {
  char* end = nullptr;
  const double parsed = std::strtod(text, &end);
  if (end == text || *end != '\0' || !std::isfinite(parsed) ||
      parsed < 0.0 || parsed > 1.0) {
    return false;
  }
  *value = static_cast<uint16_t>(std::lround(parsed * 65535.0));
  return true;
}

template <typename Voice>
int render(Voice* voice, uint16_t (&parameters)[4]) {
  voice->Init();
  voice->Configure(parameters, peaks::CONTROL_MODE_FULL);

  static peaks::GateFlags gate_flags[kFrames];
  std::memset(gate_flags, 0, sizeof(gate_flags));
  gate_flags[0] = peaks::GATE_FLAG_RISING | peaks::GATE_FLAG_HIGH;

  static int16_t samples[kFrames];
  voice->Process(gate_flags, samples, kFrames);

  for (int frame = 0; frame < kFrames; ++frame) {
    const float value = samples[frame] / 32768.0f;
    if (!std::isfinite(value)) {
      std::fputs("nonfinite upstream drum output\n", stderr);
      return 2;
    }
    if (std::printf("%.9g\n", value) < 0) {
      return 3;
    }
  }
  return std::ferror(stdout) ? 3 : 0;
}

}  // namespace

int main(int argc, char** argv) {
  if (argc != 6) {
    std::fputs(
        "expected: {bass|snare|hat|fm} p0 p1 p2 p3, each 0..1\n"
        "  bass:  frequency punch tone decay\n"
        "  snare: frequency tone snappy decay\n"
        "  hat:   (unused; the source Configure takes no controls)\n"
        "  fm:    frequency fm-amount decay noise\n",
        stderr);
    return 1;
  }
  const char* drum = argv[1];
  uint16_t parameters[4];
  for (int i = 0; i < 4; ++i) {
    if (!parse_unit(argv[2 + i], &parameters[i])) {
      std::fputs("controls must be finite and between 0 and 1\n", stderr);
      return 1;
    }
  }
  // Seed once for the drums that read stmlib::Random (snare noise, FM
  // drum noise mix); deterministic across runs of this local probe.
  stmlib::Random::Seed(1);

  if (std::strcmp(drum, "bass") == 0) {
    return render(&bass_drum, parameters);
  }
  if (std::strcmp(drum, "snare") == 0) {
    return render(&snare_drum, parameters);
  }
  if (std::strcmp(drum, "hat") == 0) {
    return render(&high_hat, parameters);
  }
  if (std::strcmp(drum, "fm") == 0) {
    return render(&fm_drum, parameters);
  }
  std::fputs("unknown drum: expected bass, snare, hat or fm\n", stderr);
  return 1;
}
