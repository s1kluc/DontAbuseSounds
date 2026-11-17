#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <mmdeviceapi.h>
#include <mmreg.h>
#include <audiomediatype.h>
#include <audioenginebaseapo.h>
#include <audioengineextensionapo.h>
#include <cassert>
#include <cmath>
#include <filesystem>
#include <fstream>
#include <vector>
#include <shlobj.h>

struct __declspec(uuid("7B1C8C67-ED98-4672-87C4-A501CB3B08C1")) DontAbuseSoundsCaptureProbe;

int main(int argc, char**) {
    const bool test_clip = argc > 1;
    std::filesystem::path folder;
    if (test_clip) {
        PWSTR path = nullptr;
        assert(SUCCEEDED(SHGetKnownFolderPath(FOLDERID_ProgramData, KF_FLAG_DONT_VERIFY, nullptr, &path)));
        folder = std::filesystem::path(path) / L"DontAbuseSounds Capture Feed";
        CoTaskMemFree(path);
        assert(std::filesystem::create_directory(folder));
    }
    const auto module = LoadLibraryW(L".\\DontAbuseSoundsCaptureProbe.dll");
    assert(module);
    using GetFactory = HRESULT (WINAPI*)(REFCLSID, REFIID, void**);
    const auto get_factory = reinterpret_cast<GetFactory>(GetProcAddress(module, "DllGetClassObject"));
    assert(get_factory);
    IClassFactory* factory = nullptr;
    assert(SUCCEEDED(get_factory(__uuidof(DontAbuseSoundsCaptureProbe), IID_PPV_ARGS(&factory))));
    IAudioProcessingObject* effect = nullptr;
    assert(SUCCEEDED(factory->CreateInstance(nullptr, IID_PPV_ARGS(&effect))));
    assert(SUCCEEDED(effect->Initialize(0, nullptr)));
    IAudioSystemEffects* system_effect = nullptr;
    assert(SUCCEEDED(effect->QueryInterface(IID_PPV_ARGS(&system_effect))));
    IAudioProcessingObjectConfiguration* configuration = nullptr;
    IAudioProcessingObjectRT* realtime = nullptr;
    assert(SUCCEEDED(effect->QueryInterface(IID_PPV_ARGS(&configuration))));
    assert(SUCCEEDED(effect->QueryInterface(IID_PPV_ARGS(&realtime))));
    WAVEFORMATEX format{WAVE_FORMAT_IEEE_FLOAT, 1, 48000, 192000, 4, 32, 0};
    IAudioMediaType* media = nullptr;
    assert(SUCCEEDED(CreateAudioMediaType(&format, sizeof(format), &media)));
    constexpr UINT32 frames = 480;
    alignas(16) float mic[frames]{}, mix[frames]{};
    APO_CONNECTION_DESCRIPTOR input{APO_CONNECTION_BUFFER_TYPE_EXTERNAL,
        reinterpret_cast<UINT_PTR>(mic), frames, media, APO_CONNECTION_DESCRIPTOR_SIGNATURE};
    APO_CONNECTION_DESCRIPTOR output{APO_CONNECTION_BUFFER_TYPE_EXTERNAL,
        reinterpret_cast<UINT_PTR>(mix), frames, media, APO_CONNECTION_DESCRIPTOR_SIGNATURE};
    auto* input_descriptor = &input;
    auto* output_descriptor = &output;
    const HRESULT lock_result = configuration->LockForProcess(1, &input_descriptor, 1, &output_descriptor);
    if (FAILED(lock_result)) {
        return static_cast<int>(lock_result);
    }

    APO_CONNECTION_PROPERTY in{reinterpret_cast<UINT_PTR>(mic), frames, BUFFER_SILENT, APO_CONNECTION_PROPERTY_SIGNATURE};
    APO_CONNECTION_PROPERTY out{reinterpret_cast<UINT_PTR>(mix), 0, BUFFER_INVALID, APO_CONNECTION_PROPERTY_SIGNATURE};
    auto* source = &in;
    auto* destination = &out;
    if (test_clip) {
        for (auto& sample : mic) sample = 0.1f;
        in.u32BufferFlags = BUFFER_VALID;
        const auto publish = [&](bool play) {
            std::ofstream file(folder / L"clip.pcm", std::ios::binary | std::ios::trunc);
            const UINT32 header[2]{0x31534144, play ? 48000u : 0u};
            file.write(reinterpret_cast<const char*>(header), sizeof(header));
            if (play) {
                const std::vector<float> clip(48000, 0.4f);
                file.write(reinterpret_cast<const char*>(clip.data()), clip.size() * sizeof(float));
            }
            assert(file.good());
        };
        const auto expect = [&](float value) {
            for (unsigned attempt = 0; attempt < 120; ++attempt) {
                realtime->APOProcess(1, &source, 1, &destination);
                if (std::abs(mix[250] - value) < 0.001f) return true;
                Sleep(10);
            }
            return false;
        };
        publish(true);
        assert(expect(0.4f));
        publish(false);
        assert(expect(0.1f));
    } else {
        realtime->APOProcess(1, &source, 1, &destination);
        assert(out.u32ValidFrameCount == frames && out.u32BufferFlags == BUFFER_VALID && mix[250] != 0.0f);
    }
    assert(SUCCEEDED(configuration->UnlockForProcess()));

    media->Release();
    realtime->Release();
    configuration->Release();
    system_effect->Release();
    effect->Release();
    factory->Release();
    FreeLibrary(module);
    if (test_clip) std::filesystem::remove_all(folder);
}
