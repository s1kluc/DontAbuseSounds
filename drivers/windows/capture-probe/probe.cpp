#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <mmdeviceapi.h>
#include <audioenginebaseapo.h>
#include <audioengineextensionapo.h>
#include <baseaudioprocessingobject.h>
#include <new>

#include "probe_tone.h"
#include "clip_feed.h"

struct __declspec(uuid("7B1C8C67-ED98-4672-87C4-A501CB3B08C1")) DontAbuseSoundsCaptureProbe;

namespace {

#pragma warning(push)
#pragma warning(disable : 4815) // CRegAPOProperties provides trailing storage for the interface ID.
const CRegAPOProperties<1> properties(
    __uuidof(DontAbuseSoundsCaptureProbe), L"DontAbuseSounds Capture Probe",
    L"DontAbuseSounds contributors", 1, 0, __uuidof(IAudioProcessingObject));
#pragma warning(pop)

LONG live_objects = 0;

class CaptureProbe final : public CBaseAudioProcessingObject, public IAudioSystemEffects {
public:
    CaptureProbe() : CBaseAudioProcessingObject(properties) {
        InterlockedIncrement(&live_objects);
    }
    ~CaptureProbe() override {
        feed_.close();
        InterlockedDecrement(&live_objects);
    }

    STDMETHODIMP QueryInterface(REFIID iid, void** out) override {
        if (!out) return E_POINTER;
        *out = nullptr;
        if (iid == __uuidof(IUnknown) || iid == __uuidof(IAudioProcessingObject))
            *out = static_cast<IAudioProcessingObject*>(this);
        else if (iid == __uuidof(IAudioProcessingObjectRT))
            *out = static_cast<IAudioProcessingObjectRT*>(this);
        else if (iid == __uuidof(IAudioProcessingObjectConfiguration))
            *out = static_cast<IAudioProcessingObjectConfiguration*>(this);
        else if (iid == __uuidof(IAudioSystemEffects))
            *out = static_cast<IAudioSystemEffects*>(this);
        else
            return E_NOINTERFACE;
        AddRef();
        return S_OK;
    }
    STDMETHODIMP_(ULONG) AddRef() override { return InterlockedIncrement(&refs_); }
    STDMETHODIMP_(ULONG) Release() override {
        const auto remaining = InterlockedDecrement(&refs_);
        if (!remaining) delete this;
        return remaining;
    }
    STDMETHODIMP Initialize(UINT32 size, BYTE* data) override {
        if (m_bIsInitialized) return APOERR_ALREADY_INITIALIZED;
        if (size && (!data || size < sizeof(APOInitBaseStruct))) return E_INVALIDARG;
        m_bIsInitialized = true;
        return S_OK;
    }
    STDMETHODIMP GetLatency(HNSTIME* latency) override {
        if (!latency) return E_POINTER;
        *latency = 0;
        return S_OK;
    }
    STDMETHODIMP LockForProcess(UINT32 inputs, APO_CONNECTION_DESCRIPTOR** in,
                                UINT32 outputs, APO_CONNECTION_DESCRIPTOR** out) override {
        const auto result = CBaseAudioProcessingObject::LockForProcess(inputs, in, outputs, out);
        if (FAILED(result)) return result;
        UNCOMPRESSEDAUDIOFORMAT format{};
        if (FAILED(in[0]->pFormat->GetUncompressedAudioFormat(&format)) ||
            format.dwBytesPerSampleContainer != sizeof(float) ||
            format.dwValidBitsPerSample != 32 ||
            format.dwSamplesPerFrame == 0 ||
            !(format.fFramesPerSecond >= 8000.0f && format.fFramesPerSecond <= 192000.0f)) {
            CBaseAudioProcessingObject::UnlockForProcess();
            return APOERR_INVALID_CONNECTION_FORMAT;
        }
        channels_ = GetSamplesPerFrame();
        feeding_ = feed_.open(format.fFramesPerSecond);
        if (!feeding_) tone_.arm(format.fFramesPerSecond);
        return S_OK;
    }
    STDMETHODIMP UnlockForProcess() override {
        const auto result = CBaseAudioProcessingObject::UnlockForProcess();
        if (SUCCEEDED(result)) { feed_.close(); feeding_ = false; }
        return result;
    }
    STDMETHODIMP Reset() override { tone_.clear(); return S_OK; }

    STDMETHODIMP_(void) APOProcess(UINT32 inputs, APO_CONNECTION_PROPERTY** in,
                                   UINT32 outputs, APO_CONNECTION_PROPERTY** out) override {
        if (inputs != 1 || outputs != 1) return;
        const auto& input = *in[0];
        auto& output = *out[0];
        output.u32ValidFrameCount = input.u32ValidFrameCount;
        output.u32BufferFlags = input.u32BufferFlags;
        if (input.u32BufferFlags != BUFFER_VALID && input.u32BufferFlags != BUFFER_SILENT) return;
        const float* mic = input.u32BufferFlags == BUFFER_VALID
            ? reinterpret_cast<const float*>(input.pBuffer) : nullptr;
        if (feeding_) {
            feed_.mix(mic, reinterpret_cast<float*>(output.pBuffer), input.u32ValidFrameCount, channels_);
            output.u32BufferFlags = BUFFER_VALID;
        } else if (tone_.mix(mic, reinterpret_cast<float*>(output.pBuffer),
                      input.u32ValidFrameCount, channels_)) {
            output.u32BufferFlags = BUFFER_VALID;
        }
    }

private:
    LONG refs_ = 1;
    UINT32 channels_ = 0;
    ProbeTone tone_;
    ClipFeed feed_;
    bool feeding_ = false;
};

class Factory final : public IClassFactory {
public:
    Factory() { InterlockedIncrement(&live_objects); }
    ~Factory() { InterlockedDecrement(&live_objects); }

    STDMETHODIMP QueryInterface(REFIID iid, void** out) override {
        if (!out) return E_POINTER;
        *out = nullptr;
        if (iid != __uuidof(IUnknown) && iid != __uuidof(IClassFactory)) return E_NOINTERFACE;
        *out = static_cast<IClassFactory*>(this);
        AddRef();
        return S_OK;
    }
    STDMETHODIMP_(ULONG) AddRef() override { return InterlockedIncrement(&refs_); }
    STDMETHODIMP_(ULONG) Release() override {
        const auto remaining = InterlockedDecrement(&refs_);
        if (!remaining) delete this;
        return remaining;
    }
    STDMETHODIMP CreateInstance(IUnknown* outer, REFIID iid, void** out) override {
        if (!out) return E_POINTER;
        *out = nullptr;
        if (outer) return CLASS_E_NOAGGREGATION;
        auto* effect = new (std::nothrow) CaptureProbe();
        if (!effect) return E_OUTOFMEMORY;
        const auto result = effect->QueryInterface(iid, out);
        effect->Release();
        return result;
    }
    STDMETHODIMP LockServer(BOOL lock) override {
        if (lock) InterlockedIncrement(&live_objects);
        else InterlockedDecrement(&live_objects);
        return S_OK;
    }

private:
    LONG refs_ = 1;
};

} // namespace

STDAPI DllGetClassObject(REFCLSID clsid, REFIID iid, void** out) {
    if (!out) return E_POINTER;
    *out = nullptr;
    if (clsid != __uuidof(DontAbuseSoundsCaptureProbe)) return CLASS_E_CLASSNOTAVAILABLE;
    auto* factory = new (std::nothrow) Factory();
    if (!factory) return E_OUTOFMEMORY;
    const auto result = factory->QueryInterface(iid, out);
    factory->Release();
    return result;
}

STDAPI DllCanUnloadNow() {
    return InterlockedCompareExchange(&live_objects, 0, 0) == 0 ? S_OK : S_FALSE;
}
