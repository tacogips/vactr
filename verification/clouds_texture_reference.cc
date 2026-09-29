// Local reference driver for the pinned Clouds GranularProcessor.
// This original probe includes source headers and links the separate checkout;
// it contains no copied Mutable Instruments implementation or audio data.

#include <cmath>
#include <algorithm>
#include <cstdio>
#include <cstdlib>
#include <cstring>

#include "clouds/dsp/granular_processor.h"
#include "clouds/dsp/parameters.h"

namespace {
constexpr int kBlock = 32;
constexpr int kRate = 32000;
constexpr int kFrames = kRate * 3;
clouds::GranularProcessor processor;
uint8_t large_buffer[118784];
uint8_t small_buffer[65536 - 128];

struct Controls {
  float position;
  float size;
  float pitch;
  float density;
  float texture;
  float dry_wet;
  float stereo_spread;
  float feedback;
  float reverb;
};

constexpr Controls kScenarios[] = {
    {0.15f, 0.18f, -7.0f, 0.25f, 0.22f, 0.70f, 0.0f, 0.15f, 0.10f},
    {0.50f, 0.52f, 0.0f, 0.50f, 0.50f, 0.80f, 0.50f, 0.45f, 0.35f},
    {0.82f, 0.86f, 7.0f, 0.78f, 0.82f, 0.90f, 1.0f, 0.72f, 0.65f},
};

bool integer_arg(const char* text, int min, int max, int* value) {
  char* end = nullptr;
  const long parsed = std::strtol(text, &end, 10);
  if (end == text || *end != '\0' || parsed < min || parsed > max) return false;
  *value = static_cast<int>(parsed);
  return true;
}

bool read_input(clouds::ShortFrame* input) {
  for (int i = 0; i < kBlock; ++i) {
    float left;
    float right;
    if (std::scanf("%f %f", &left, &right) != 2 || !std::isfinite(left) ||
        !std::isfinite(right)) {
      return false;
    }
    const float bounded_left = std::max(-1.0f, std::min(1.0f, left));
    const float bounded_right = std::max(-1.0f, std::min(1.0f, right));
    input[i].l = static_cast<int16_t>(bounded_left * 32767.0f);
    input[i].r = static_cast<int16_t>(bounded_right * 32767.0f);
  }
  return true;
}
}  // namespace

int main(int argc, char** argv) {
  int mode = 0;
  int scenario = 0;
  int freeze = 0;
  int quality = 0;
  if (argc != 5 || !integer_arg(argv[1], 0, 3, &mode) ||
      !integer_arg(argv[2], 0, 2, &scenario) ||
      !integer_arg(argv[3], 0, 1, &freeze) ||
      !integer_arg(argv[4], 0, 3, &quality)) {
    std::fputs("expected mode(0..3) scenario(0..2) freeze(0|1) quality(0..3)\n",
               stderr);
    return 1;
  }

  processor.Init(large_buffer, sizeof(large_buffer), small_buffer,
                 sizeof(small_buffer));
  processor.set_playback_mode(static_cast<clouds::PlaybackMode>(mode));
  processor.set_quality(quality);
  clouds::Parameters* p = processor.mutable_parameters();
  const Controls c = kScenarios[scenario];
  p->position = c.position;
  p->size = c.size;
  p->pitch = c.pitch;
  p->density = c.density;
  p->texture = c.texture;
  p->dry_wet = c.dry_wet;
  p->stereo_spread = c.stereo_spread;
  p->feedback = c.feedback;
  p->reverb = c.reverb;
  p->freeze = false;
  p->trigger = false;
  p->gate = true;
  processor.Prepare();

  for (int offset = 0; offset < kFrames; offset += kBlock) {
    clouds::ShortFrame input[kBlock];
    clouds::ShortFrame output[kBlock];
    if (!read_input(input)) {
      std::fputs("input ended before 3 seconds of complete 32-frame blocks\n",
                 stderr);
      return 2;
    }
    p->freeze = freeze != 0 && offset >= kRate;
    p->trigger = (offset / kBlock) % 32 == 0;
    processor.Prepare();
    processor.Process(input, output, kBlock);
    for (int i = 0; i < kBlock; ++i) {
      const float left = static_cast<float>(output[i].l) / 32768.0f;
      const float right = static_cast<float>(output[i].r) / 32768.0f;
      if (!std::isfinite(left) || !std::isfinite(right)) {
        std::fputs("nonfinite Clouds output\n", stderr);
        return 3;
      }
      if (std::printf("%.9g %.9g\n", left, right) < 0) return 4;
    }
  }
  return std::ferror(stdout) ? 4 : 0;
}
