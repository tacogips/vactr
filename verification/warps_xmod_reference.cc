// Local raw-kernel probe for the pinned MIT Warps modulator. The upstream
// sources are included only from a separate checkout at build time.

#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <vector>

#include "warps/dsp/modulator.h"

namespace {

constexpr size_t kBlockSize = 60;
constexpr float kScale = 32768.0f;

bool ParseFloat(const char* text, float* value) {
  char* end = nullptr;
  *value = std::strtof(text, &end);
  return end != text && *end == '\0' && std::isfinite(*value);
}

bool ParseSize(const char* text, size_t* value) {
  char* end = nullptr;
  const unsigned long parsed = std::strtoul(text, &end, 10);
  if (end == text || *end != '\0' || parsed == 0) {
    return false;
  }
  *value = static_cast<size_t>(parsed);
  return true;
}

short ToShort(float sample) {
  const int32_t scaled = static_cast<int32_t>(sample * kScale);
  const int32_t clipped = scaled < -32768 ? -32768 :
      (scaled > 32767 ? 32767 : scaled);
  return static_cast<short>(clipped);
}

}  // namespace

int main(int argc, char** argv) {
  if (argc != 9) {
    std::fprintf(stderr,
        "usage: warps_xmod_reference POSITION TIMBRE CARRIER_DRIVE "
        "MODULATOR_DRIVE CARRIER_SHAPE NOTE INPUT_F32_STEREO FRAMES\n");
    return 2;
  }

  float position = 0.0f;
  float timbre = 0.0f;
  float carrier_drive = 0.0f;
  float modulator_drive = 0.0f;
  float note = 0.0f;
  size_t frames = 0;
  if (!ParseFloat(argv[1], &position) || position < 0.0f || position > 5.999f ||
      !ParseFloat(argv[2], &timbre) || timbre < 0.0f || timbre > 1.0f ||
      !ParseFloat(argv[3], &carrier_drive) || carrier_drive < 0.0f || carrier_drive > 1.0f ||
      !ParseFloat(argv[4], &modulator_drive) || modulator_drive < 0.0f || modulator_drive > 1.0f ||
      !ParseFloat(argv[6], &note) || !ParseSize(argv[8], &frames) ||
      (frames % kBlockSize) != 0) {
    std::fprintf(stderr, "invalid controls or frame count\n");
    return 2;
  }
  char* shape_end = nullptr;
  const long shape = std::strtol(argv[5], &shape_end, 10);
  if (shape_end == argv[5] || *shape_end != '\0' || shape < 0 || shape > 3) {
    std::fprintf(stderr, "carrier shape must be 0..3\n");
    return 2;
  }

  FILE* input_file = std::fopen(argv[7], "rb");
  if (input_file == nullptr) {
    std::perror("open input");
    return 2;
  }
  std::vector<warps::ShortFrame> input(frames);
  std::vector<warps::ShortFrame> output(frames);
  for (size_t i = 0; i < frames; ++i) {
    float samples[2] = {};
    if (std::fread(samples, sizeof(float), 2, input_file) != 2 ||
        !std::isfinite(samples[0]) || !std::isfinite(samples[1])) {
      std::fprintf(stderr, "input must contain finite interleaved stereo f32 samples\n");
      std::fclose(input_file);
      return 2;
    }
    input[i].l = ToShort(samples[0]);
    input[i].r = ToShort(samples[1]);
  }
  if (std::fgetc(input_file) != EOF) {
    std::fprintf(stderr, "input contains more frames than requested\n");
    std::fclose(input_file);
    return 2;
  }
  std::fclose(input_file);

  static warps::Modulator modulator;
  modulator.Init(96000.0f);
  warps::Parameters* parameters = modulator.mutable_parameters();
  parameters->modulation_algorithm = position / 8.0f;
  parameters->modulation_parameter = timbre;
  parameters->channel_drive[0] = carrier_drive;
  parameters->channel_drive[1] = modulator_drive;
  parameters->carrier_shape = static_cast<int32_t>(shape);
  parameters->note = note;

  for (size_t offset = 0; offset < frames; offset += kBlockSize) {
    modulator.Process(&input[offset], &output[offset], kBlockSize);
  }
  for (size_t i = 0; i < frames; ++i) {
    const float main = static_cast<float>(output[i].l) / kScale;
    const float aux = static_cast<float>(output[i].r) / kScale;
    if (!std::isfinite(main) || !std::isfinite(aux)) {
      std::fprintf(stderr, "upstream emitted non-finite audio\n");
      return 1;
    }
    std::printf("%.9g %.9g\n", main, aux);
  }
  return 0;
}
