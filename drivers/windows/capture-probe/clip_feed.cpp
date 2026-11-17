#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <shlobj.h>
#include <algorithm>
#include <cmath>
#include <filesystem>
#include <memory>
#include <string>
#include <utility>
#include "clip_feed.h"

namespace {
constexpr std::uint32_t wire_magic = 0x31534144; // DAS1
constexpr std::uint32_t frame_limit = 48000 * 60;

std::uint64_t ticks(FILETIME time) {
    return (static_cast<std::uint64_t>(time.dwHighDateTime) << 32) | time.dwLowDateTime;
}

bool read(HANDLE file, void* data, DWORD size) {
    DWORD count = 0;
    return ReadFile(file, data, size, &count, nullptr) && count == size;
}
}

bool ClipFeed::open(float endpoint_rate) {
    PWSTR path = nullptr;
    if (FAILED(SHGetKnownFolderPath(FOLDERID_ProgramData, KF_FLAG_DONT_VERIFY, nullptr, &path)))
        return false;
    const auto free_path = [](wchar_t* ptr) { CoTaskMemFree(ptr); };
    std::unique_ptr<wchar_t, decltype(free_path)> root(path, free_path);
    try {
        const auto file = std::filesystem::path(root.get()) / L"DontAbuseSounds Capture Feed" / L"clip.pcm";
        if (!std::filesystem::is_directory(file.parent_path())) return false;
        step_ = 48000.0 / endpoint_rate;
        quit_.store(false);
        worker_ = std::thread([this, file] { receive(file.c_str()); });
        return true;
    } catch (...) {
        return false;
    }
}

void ClipFeed::close() {
    quit_.store(true);
    if (worker_.joinable()) worker_.join();
    delete pending_.exchange(nullptr);
    delete retired_.exchange(nullptr);
    delete std::exchange(current_, nullptr);
    position_ = 0;
}

void ClipFeed::receive(const wchar_t* path) {
    std::uint64_t last = 0;
    while (!quit_.load()) {
        delete retired_.exchange(nullptr);
        WIN32_FILE_ATTRIBUTE_DATA attributes{};
        if (GetFileAttributesExW(path, GetFileExInfoStandard, &attributes) &&
            !(attributes.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) &&
            ticks(attributes.ftLastWriteTime) != last) {
            const HANDLE file = CreateFileW(path, GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_DELETE, nullptr,
                                            OPEN_EXISTING, FILE_FLAG_OPEN_REPARSE_POINT, nullptr);
            if (file != INVALID_HANDLE_VALUE) {
                FILETIME written{}, now{};
                BY_HANDLE_FILE_INFORMATION info{};
                GetSystemTimePreciseAsFileTime(&now);
                if (GetFileInformationByHandle(file, &info) &&
                    !(info.dwFileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY)) &&
                    GetFileTime(file, nullptr, nullptr, &written)) {
                    last = ticks(written);
                    std::uint32_t header[2]{};
                    // Ignore old clips when the application opens the recording stream again.
                    if (ticks(now) >= last && ticks(now) - last <= 20000000 &&
                        read(file, header, sizeof(header)) && header[0] == wire_magic && header[1] <= frame_limit &&
                        info.nFileSizeHigh == 0 && info.nFileSizeLow == sizeof(header) + header[1] * sizeof(float)) {
                        try {
                            auto next = std::make_unique<Clip>(header[1]);
                            if (next->empty() || read(file, next->data(), header[1] * sizeof(float))) {
                                for (float& sample : *next)
                                    sample = std::isfinite(sample) ? std::clamp(sample, -1.0f, 1.0f) : 0.0f;
                                delete pending_.exchange(next.release());
                            }
                        } catch (const std::bad_alloc&) {
                            // A failed clip allocation leaves the microphone path active.
                        }
                    }
                }
                CloseHandle(file);
            }
        }
        Sleep(20);
    }
}

void ClipFeed::mix(const float* mic, float* out, std::uint32_t frames, std::uint32_t channels) noexcept {
    if (!retired_.load()) {
        if (auto* next = pending_.exchange(nullptr)) {
            retired_.store(std::exchange(current_, next));
            position_ = 0;
        }
    }
    for (std::uint32_t i = 0; i < frames; ++i) {
        const bool play = current_ && position_ < static_cast<double>(current_->size());
        float clip = 0.0f;
        if (play) {
            const auto index = static_cast<std::size_t>(position_);
            const auto next = std::min(index + 1, current_->size() - 1);
            clip = 0.75f * ((*current_)[index] + ((*current_)[next] - (*current_)[index]) *
                             static_cast<float>(position_ - index));
            position_ += step_;
        }
        for (std::uint32_t c = 0; c < channels; ++c) {
            const float voice = mic ? mic[c] : 0.0f;
            out[c] = play ? std::clamp(voice + clip, -1.0f, 1.0f) : voice;
        }
        if (mic) mic += channels;
        out += channels;
    }
}
