// Probe harness for the pinned Plaits voice and low-pass gate behavior.
// Upstream source is compiled from a separate checkout and is never modified.

#include <cmath>
#include <algorithm>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <iostream>
#include <string>
#include <vector>

#include "plaits/dsp/voice.h"
#include "plaits/dsp/fx/low_pass_gate.h"
#include "plaits/dsp/envelope.h"
#include "stmlib/dsp/limiter.h"
#include "stmlib/utils/buffer_allocator.h"

namespace {

const size_t kBlock = 12;
const size_t kAlignBlock = 20;

bool ReadFloats(const std::string& path, std::vector<float>* values) {
  std::ifstream input(path.c_str());
  if (!input) return false;
  float value;
  while (input >> value) {
    if (!std::isfinite(value)) return false;
    values->push_back(value);
  }
  return input.eof();
}

bool ReadTrajectory(const std::string& path,
                    std::vector<float>* gain,
                    std::vector<float>* frequency,
                    std::vector<float>* bleed) {
  std::ifstream input(path.c_str());
  if (!input) return false;
  while (input) {
    float g;
    if (!(input >> g)) break;
    float f;
    float b;
    if (!(input >> f >> b) || !std::isfinite(g) || !std::isfinite(f) ||
        !std::isfinite(b)) return false;
    gain->push_back(g);
    frequency->push_back(f);
    bleed->push_back(b);
  }
  return input.eof();
}

bool ParseMode(const std::string& text, bool* trigger_patched,
               bool* level_patched) {
  if (text == "off") {
    *trigger_patched = false;
    *level_patched = false;
    return true;
  }
  if (text == "ping") {
    *trigger_patched = true;
    *level_patched = false;
    return true;
  }
  if (text == "level") {
    *trigger_patched = true;
    *level_patched = true;
    return true;
  }
  return false;
}

int RunVoice(int argc, char** argv) {
  if (argc != 10) return 2;
  const int engine = std::atoi(argv[2]);
  bool trigger_patched = false;
  bool level_patched = false;
  if (!ParseMode(argv[3], &trigger_patched, &level_patched)) return 2;
  const float decay = static_cast<float>(std::atof(argv[4]));
  const float color = static_cast<float>(std::atof(argv[5]));
  const float note = static_cast<float>(std::atof(argv[6]));
  const float velocity = static_cast<float>(std::atof(argv[7]));
  const int gate_blocks = std::atoi(argv[8]);
  const int blocks = std::atoi(argv[9]);
  if (engine < 0 || engine >= plaits::kMaxEngines || gate_blocks < 1 ||
      blocks < 1 || blocks > 100000) return 2;

  static unsigned char memory[32768];
  stmlib::BufferAllocator allocator(memory, sizeof(memory));
  plaits::Voice voice;
  voice.Init(&allocator);
  plaits::Patch patch = {};
  patch.engine = engine;
  patch.note = note;
  patch.harmonics = 0.5f;
  patch.timbre = 0.5f;
  patch.morph = 0.5f;
  patch.decay = decay;
  patch.lpg_colour = color;
  plaits::Modulations modulations = {};
  modulations.note = 0.0f;
  modulations.engine = 0.0f;
  modulations.trigger_patched = trigger_patched;
  modulations.level_patched = level_patched;
  plaits::Voice::Frame frames[kBlock];
  bool checked_alignment = false;

  for (int block = 0; block < static_cast<int>(kAlignBlock) + blocks; ++block) {
    modulations.trigger = (block >= 16 && block < 16 + gate_blocks) ? 1.0f : 0.0f;
    modulations.level = (block >= 20 && block < 20 + gate_blocks) ? velocity : 0.0f;
    voice.Render(patch, modulations, frames, kBlock);
    if (trigger_patched && block == 19 && voice.trigger_state_) {
      std::cerr << "trigger_state_ unexpectedly true after block 19\n";
      return 3;
    }
    if (trigger_patched && block == 20) {
      if (!voice.trigger_state_) {
        std::cerr << "trigger_state_ unexpectedly false after block 20\n";
        return 3;
      }
      checked_alignment = true;
    }
    if (block < static_cast<int>(kAlignBlock)) continue;
    if (block >= static_cast<int>(kAlignBlock) + blocks) break;
    std::printf("T %d %.9g %.9g %.9g %.9g %d\n", block - 20,
                voice.lpg_envelope_.gain(), voice.lpg_envelope_.frequency(),
                voice.lpg_envelope_.hf_bleed(), voice.decay_envelope_.value(),
                (block - 20) * static_cast<int>(kBlock));
    for (size_t i = 0; i < kBlock; ++i) {
      const float main = -((static_cast<float>(frames[i].out) - 1.0f) / 32767.0f);
      const float aux = -((static_cast<float>(frames[i].aux) - 1.0f) / 32767.0f);
      std::printf("M %.9g\nX %.9g\nR %.9g\nQ %.9g\n", main, aux,
                  voice.out_buffer_[i], voice.aux_buffer_[i]);
    }
  }
  if (trigger_patched && !checked_alignment) {
    std::cerr << "trigger alignment block was not rendered\n";
    return 3;
  }
  return 0;
}

int RunAudio(int argc, char** argv) {
  if (argc != 5) return 2;
  const float registered_gain = static_cast<float>(std::atof(argv[2]));
  std::vector<float> input;
  std::vector<float> gain;
  std::vector<float> frequency;
  std::vector<float> bleed;
  if (!ReadFloats(argv[3], &input) ||
      !ReadTrajectory(argv[4], &gain, &frequency, &bleed)) return 2;
  if (gain.size() * kBlock < input.size()) return 2;

  plaits::LowPassGate lpg;
  stmlib::Limiter limiter;
  lpg.Init();
  limiter.Init();
  const float post = registered_gain < 0.0f ? 1.0f : registered_gain;
  for (size_t offset = 0, block = 0; offset < input.size(); offset += kBlock, ++block) {
    const size_t size = std::min(kBlock, input.size() - offset);
    float samples[kBlock];
    for (size_t i = 0; i < size; ++i) samples[i] = input[offset + i];
    if (registered_gain < 0.0f) limiter.Process(-registered_gain, samples, size);
    lpg.Process(post * gain[block], frequency[block], bleed[block], samples, size);
    for (size_t i = 0; i < size; ++i) {
      const float value = std::max(-1.0f, std::min(1.0f, samples[i]));
      std::printf("M %.9g\n", value);
    }
  }
  return 0;
}

int RunBypass(int argc, char** argv) {
  if (argc != 4) return 2;
  const float registered_gain = static_cast<float>(std::atof(argv[2]));
  std::vector<float> input;
  if (!ReadFloats(argv[3], &input)) return 2;
  stmlib::Limiter limiter;
  limiter.Init();
  for (size_t offset = 0; offset < input.size(); offset += kBlock) {
    const size_t size = std::min(kBlock, input.size() - offset);
    float samples[kBlock];
    for (size_t i = 0; i < size; ++i) samples[i] = input[offset + i];
    if (registered_gain < 0.0f) limiter.Process(-registered_gain, samples, size);
    const float post = registered_gain < 0.0f ? 1.0f : registered_gain;
    for (size_t i = 0; i < size; ++i) {
      const float value = std::max(-1.0f, std::min(1.0f, samples[i] * post));
      std::printf("M %.9g\n", value);
    }
  }
  return 0;
}

}  // namespace

int main(int argc, char** argv) {
  if (argc < 2) return 2;
  const std::string command(argv[1]);
  if (command == "voice") return RunVoice(argc, argv);
  if (command == "audio") return RunAudio(argc, argv);
  if (command == "bypass") return RunBypass(argc, argv);
  return 2;
}
