// Local reference driver for a separate pinned Eurorack checkout.
// This original file contains no Mutable Instruments DSP or resource data.
// It is compiled to a temporary executable and never linked into Vactr.

#include <cmath>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>

#include "plaits/dsp/engine/modal_engine.h"
#include "plaits/dsp/engine/noise_engine.h"
#include "plaits/dsp/engine/particle_engine.h"
#include "plaits/dsp/engine/string_engine.h"
#include "plaits/dsp/engine/swarm_engine.h"
#include "stmlib/utils/random.h"

namespace {
constexpr int kBlock = 24;
constexpr int kBlocks = 3000;
constexpr float kNote = 69.0f;
constexpr size_t kAllocatorBytes = 262144;

alignas(std::max_align_t) uint8_t modal_storage[kAllocatorBytes];
alignas(std::max_align_t) uint8_t noise_storage[kAllocatorBytes];
alignas(std::max_align_t) uint8_t particle_storage[kAllocatorBytes];
alignas(std::max_align_t) uint8_t string_storage[kAllocatorBytes];
alignas(std::max_align_t) uint8_t swarm_storage[kAllocatorBytes];
plaits::ModalEngine modal_engine;
plaits::NoiseEngine noise_engine;
plaits::ParticleEngine particle_engine;
plaits::StringEngine string_engine;
plaits::SwarmEngine swarm_engine;

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

plaits::Engine* initialize(const char* name) {
  if (std::strcmp(name, "swarm") == 0) {
    stmlib::BufferAllocator allocator(swarm_storage, sizeof(swarm_storage));
    swarm_engine.Init(&allocator);
    return &swarm_engine;
  }
  if (std::strcmp(name, "noise") == 0) {
    stmlib::BufferAllocator allocator(noise_storage, sizeof(noise_storage));
    noise_engine.Init(&allocator);
    return &noise_engine;
  }
  if (std::strcmp(name, "particle") == 0) {
    stmlib::BufferAllocator allocator(particle_storage, sizeof(particle_storage));
    particle_engine.Init(&allocator);
    return &particle_engine;
  }
  if (std::strcmp(name, "string") == 0) {
    stmlib::BufferAllocator allocator(string_storage, sizeof(string_storage));
    string_engine.Init(&allocator);
    return &string_engine;
  }
  if (std::strcmp(name, "modal") == 0) {
    stmlib::BufferAllocator allocator(modal_storage, sizeof(modal_storage));
    modal_engine.Init(&allocator);
    return &modal_engine;
  }
  return nullptr;
}
}  // namespace

int main(int argc, char** argv) {
  if (argc != 6) {
    std::fputs("expected: engine(swarm|noise|particle|string|modal) harmonics timbre morph accent\n", stderr);
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

  stmlib::Random::Seed(0x51A7E123);
  plaits::Engine* engine = initialize(argv[1]);
  if (engine == nullptr) {
    std::fputs("unknown Plaits engine\n", stderr);
    return 1;
  }
  engine->Reset();

  plaits::EngineParameters parameters{};
  parameters.note = kNote;
  parameters.harmonics = harmonics;
  parameters.timbre = timbre;
  parameters.morph = morph;
  parameters.accent = accent;

  for (int block = 0; block < kBlocks; ++block) {
    parameters.trigger = block == 0
        ? plaits::TRIGGER_RISING_EDGE
        : plaits::TRIGGER_LOW;
    float main_out[kBlock];
    float aux_out[kBlock];
    bool already_enveloped = false;
    engine->Render(parameters, main_out, aux_out, kBlock, &already_enveloped);
    for (int frame = 0; frame < kBlock; ++frame) {
      if (!std::isfinite(main_out[frame]) || !std::isfinite(aux_out[frame])) {
        std::fputs("nonfinite upstream output\n", stderr);
        return 2;
      }
      if (std::printf("%.9g %.9g\n", main_out[frame], aux_out[frame]) < 0) {
        return 3;
      }
    }
  }
  return std::ferror(stdout) ? 3 : 0;
}
