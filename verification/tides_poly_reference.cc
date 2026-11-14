// Original local driver for comparison against a separate pinned checkout.
// It imports no upstream code or tables into Vactr.

#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>

#include "stmlib/utils/gate_flags.h"
#ifdef TIDES1_PROBE
#include "tides/generator.h"
#else
#include "tides2/poly_slope_generator.h"
#endif

namespace {
constexpr size_t kFrames = 24000;
constexpr size_t kBlock = 24;

bool number(const char* text, float* value) {
  char* end = nullptr;
  *value = std::strtof(text, &end);
  return end != text && *end == '\0' && std::isfinite(*value);
}

bool integer(const char* text, int* value) {
  char* end = nullptr;
  long parsed = std::strtol(text, &end, 10);
  if (end == text || *end != '\0' || parsed < 0 || parsed > 100) return false;
  *value = static_cast<int>(parsed);
  return true;
}

#ifndef TIDES1_PROBE
int render_tides2(int argc, char** argv) {
  if (argc != 10) return 2;
  int mode, output, range;
  float frequency, pw, shape, smoothness, shift;
  if (!integer(argv[2], &mode) || !integer(argv[3], &output) ||
      !integer(argv[4], &range) || !number(argv[5], &frequency) ||
      !number(argv[6], &pw) || !number(argv[7], &shape) ||
      !number(argv[8], &smoothness) || !number(argv[9], &shift) ||
      mode > 2 || output > 3 || range > 1 || frequency <= 0.0f ||
      pw < 0.0f || pw > 1.0f || shape < 0.0f ||
      shape > 1.0f || smoothness < 0.0f || smoothness > 1.0f ||
      shift < 0.0f || shift > 1.0f) {
    std::fputs("invalid Tides2 mode/output/range/settings\n", stderr);
    return 2;
  }
  tides::PolySlopeGenerator generator;
  generator.Init();
  tides::PolySlopeGenerator::OutputSample block[kBlock];
  stmlib::GateFlags gates[kBlock];
  const size_t render_rate_divider = output >= 2 ? 2 : 1;
  if (frequency >= 0.25f * (48000.0f / render_rate_divider)) return 2;
  const size_t blocks = kFrames / (kBlock * render_rate_divider);
  for (size_t b = 0; b < blocks; ++b) {
    std::fill(gates, gates + kBlock, stmlib::GATE_FLAG_LOW);
    if (b == 0) {
      gates[0] = static_cast<stmlib::GateFlags>(stmlib::GATE_FLAG_RISING |
                                                  stmlib::GATE_FLAG_HIGH);
      std::fill(gates + 1, gates + kBlock / render_rate_divider,
                stmlib::GATE_FLAG_HIGH);
    }
    generator.Render(
        static_cast<tides::RampMode>(mode),
        static_cast<tides::OutputMode>(output),
        range == 0 ? tides::RANGE_CONTROL : tides::RANGE_AUDIO,
        frequency / (48000.0f / static_cast<float>(render_rate_divider)),
        pw, shape, smoothness, shift, gates, nullptr, block, kBlock);
    for (size_t i = 0; i < kBlock; ++i) {
      for (size_t repeat = 0; repeat < render_rate_divider; ++repeat) {
        for (int lane = 0; lane < 4; ++lane) {
          const float sample = block[i].channel[lane];
          if (!std::isfinite(sample)) return 3;
          if (std::printf("%.9g%c", sample, lane == 3 ? '\n' : ' ') < 0) return 4;
        }
      }
    }
  }
  return std::ferror(stdout) ? 4 : 0;
}
#endif

#ifdef TIDES1_PROBE
int render_tides1(int argc, char** argv) {
  if (argc != 7) return 2;
  int mode;
  float frequency, shape, slope, smoothness;
  if (!integer(argv[2], &mode) || !number(argv[3], &frequency) ||
      !number(argv[4], &shape) || !number(argv[5], &slope) ||
      !number(argv[6], &smoothness) || mode > 2 || frequency <= 0.0f ||
      frequency >= 20000.0f || shape < 0.0f || shape > 1.0f || slope < 0.0f ||
      slope > 1.0f || smoothness < 0.0f || smoothness > 1.0f) {
    std::fputs("invalid Tides1 mode/settings\n", stderr);
    return 2;
  }
  tides::Generator generator;
  generator.Init();
  generator.set_range(tides::GENERATOR_RANGE_HIGH);
  generator.set_mode(static_cast<tides::GeneratorMode>(mode));
  const float pitch = (12.0f * std::log2(frequency / 8.1757989f) - 12.0f) * 128.0f;
  generator.set_pitch(static_cast<int16_t>(std::round(pitch)));
  generator.set_shape(static_cast<int16_t>(std::round((shape * 2.0f - 1.0f) * 32767.0f)));
  generator.set_slope(static_cast<int16_t>(std::round((slope * 2.0f - 1.0f) * 32767.0f)));
  generator.set_smoothness(static_cast<int16_t>(std::round((smoothness * 2.0f - 1.0f) * 32767.0f)));

  size_t emitted = 0;
  while (emitted < kFrames) {
    const size_t count = std::min(kBlock, kFrames - emitted);
    for (size_t i = 0; i < count; ++i) {
      uint8_t control = tides::CONTROL_FREEZE;
      if (emitted + i < kBlock) {
        control = emitted + i == 0
            ? tides::CONTROL_GATE_RISING | tides::CONTROL_GATE
            : tides::CONTROL_GATE;
      } else {
        control = 0;
      }
      const tides::GeneratorSample& sample = generator.Process(control);
      if (generator.writable_block()) generator.Process();
      const float uni = sample.unipolar / 65535.0f;
      const float bi = sample.bipolar / 32767.0f;
      if (std::printf("%.9g %.9g\n", uni, bi) < 0) return 4;
    }
    emitted += count;
  }
  return std::ferror(stdout) ? 4 : 0;
}
#endif
}  // namespace

int main(int argc, char** argv) {
#ifdef TIDES1_PROBE
  if (argc < 2 || std::strcmp(argv[1], "tides1") != 0) return 2;
  return render_tides1(argc, argv);
#else
  if (argc < 2) return 2;
  if (std::strcmp(argv[1], "tides2") == 0) return render_tides2(argc, argv);
  return 2;
#endif
}
