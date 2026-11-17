#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <mmdeviceapi.h>
#include <mmreg.h>
#include <audiomediatype.h>
#include <audioenginebaseapo.h>
#include <audioengineextensionapo.h>
#include <cassert>

struct __declspec(uuid("7B1C8C67-ED98-4672-87C4-A501CB3B08C1")) DontAbuseSoundsCaptureProbe;

int main() {
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
    realtime->APOProcess(1, &source, 1, &destination);
    assert(out.u32ValidFrameCount == frames && out.u32BufferFlags == BUFFER_VALID && mix[250] != 0.0f);
    assert(SUCCEEDED(configuration->UnlockForProcess()));

    media->Release();
    realtime->Release();
    configuration->Release();
    system_effect->Release();
    effect->Release();
    factory->Release();
    FreeLibrary(module);
}
