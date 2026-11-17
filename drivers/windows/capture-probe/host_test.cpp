#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <mmdeviceapi.h>
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
    system_effect->Release();
    effect->Release();
    factory->Release();
    FreeLibrary(module);
}
