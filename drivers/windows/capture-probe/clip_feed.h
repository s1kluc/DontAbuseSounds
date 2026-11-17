#pragma once

#include <atomic>
#include <cstdint>
#include <thread>
#include <vector>

class ClipFeed {
public:
    ~ClipFeed() { close(); }
    bool open(float endpoint_rate);
    void close();
    void mix(const float* mic, float* out, std::uint32_t frames, std::uint32_t channels) noexcept;

private:
    using Clip = std::vector<float>;
    void receive(const wchar_t* path);

    std::atomic<bool> quit_{false};
    std::atomic<Clip*> pending_{nullptr}, retired_{nullptr};
    std::thread worker_;
    Clip* current_ = nullptr;
    double step_ = 1, position_ = 0;
};
