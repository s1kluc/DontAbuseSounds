#pragma once

#include <algorithm>
#include <cmath>
#include <cstdint>

class ProbeTone {
public:
    void arm(float rate) noexcept {
        remaining_ = length_ = static_cast<std::uint32_t>(rate / 2);
        fade_ = static_cast<std::uint32_t>(rate / 200);
        constexpr float tau = 6.28318530718f;
        const float step = tau * 440.0f / rate;
        sine_step_ = std::sin(step);
        cosine_step_ = std::cos(step);
        sine_ = 0.0f;
        cosine_ = 1.0f;
    }

    void clear() noexcept { remaining_ = 0; }

    // Returns whether the block contains tone, including when the microphone is silent.
    bool mix(const float* mic, float* out, std::uint32_t frames, std::uint32_t channels) noexcept {
        const bool has_tone = frames && remaining_;
        for (std::uint32_t i = 0; i < frames; ++i) {
            if (remaining_) {
                const auto ramp = std::min({length_ - remaining_, remaining_, fade_});
                const float tone = sine_ * (0.12f * static_cast<float>(ramp) / static_cast<float>(fade_));
                const float next_sine = sine_ * cosine_step_ + cosine_ * sine_step_;
                cosine_ = cosine_ * cosine_step_ - sine_ * sine_step_;
                sine_ = next_sine;
                --remaining_;
                for (std::uint32_t c = 0; c < channels; ++c)
                    out[c] = std::clamp((mic ? mic[c] : 0.0f) + tone, -1.0f, 1.0f);
            } else {
                for (std::uint32_t c = 0; c < channels; ++c)
                    out[c] = mic ? mic[c] : 0.0f;
            }
            if (mic) mic += channels;
            out += channels;
        }
        return has_tone;
    }

private:
    std::uint32_t length_ = 0, remaining_ = 0, fade_ = 0;
    float sine_ = 0.0f, cosine_ = 1.0f;
    float sine_step_ = 0.0f, cosine_step_ = 1.0f;
};
