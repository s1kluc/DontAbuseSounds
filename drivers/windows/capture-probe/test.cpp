#include "probe_tone.h"
#include <cassert>
#include <vector>

int main() {
    ProbeTone tone;
    tone.arm(48000);
    std::vector<float> block(24000 * 2, 0.25f);
    assert(tone.mix(block.data(), block.data(), 24000, 2));
    assert(block[2002] != 0.25f && block[2002] == block[2003]);
    for (const float sample : block) assert(sample >= 0.12f && sample <= 0.38f);
    float unchanged[] = {0.25f, -0.25f};
    assert(!tone.mix(unchanged, unchanged, 1, 2));
    assert(unchanged[0] == 0.25f && unchanged[1] == -0.25f);

    tone.arm(48000);
    std::vector<float> silence(480, 8.0f);
    assert(tone.mix(nullptr, silence.data(), 480, 1));
    assert(silence[0] == 0.0f && silence[250] != 0.0f);
    tone.clear();
    assert(!tone.mix(nullptr, silence.data(), 480, 1));
    for (const float sample : silence) assert(sample == 0.0f);
}
