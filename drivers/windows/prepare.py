"""Adapt the pinned Microsoft SimpleAudioSample to a stereo PCM feed/mic pair."""

from pathlib import Path
import sys

sample = Path(sys.argv[1])
main = sample / "Source/Main"
filters = sample / "Source/Filters"


def replace_once(text, old, new):
    if text.count(old) != 1:
        raise ValueError(f"Expected one occurrence of {old!r}")
    return text.replace(old, new, 1)


def edit(path, transform, encoding="utf-8"):
    text = path.read_text(encoding=encoding)
    path.write_text(transform(text), encoding=encoding)


def capture_format(text):
    return (
        text.replace("32_BITS_PER_SAMPLE_PCM", "16_BITS_PER_SAMPLE_PCM")
        .replace("32      // 32 Bits Per Sample", "16      // 16 Bits Per Sample")
        .replace("32-bit", "16-bit")
        .replace("384000,", "192000,")
        .replace("                8,", "                4,")
        .replace("                32,", "                16,")
        .replace("            32,", "            16,")
    )


edit(filters / "micarraywavtable.h", capture_format)


def inf(text):
    for old, new in {
        r"ROOT\SimpleAudioSample": r"ROOT\DontAbuseSoundsMic",
        "02/22/2016, 1.0.0.1": "01/01/2026, 0.2.0.0",
        "Virtual Audio Device (WDM) - Simple Audio Sample Driver": "DontAbuseSounds Audio Driver",
        "Virtual Audio Device (WDM) - Simple Audio Sample": "DontAbuseSounds Virtual Audio",
        "TODO-Set-Provider": "DontAbuseSounds",
        "TODO-Set-Manufacturer": "DontAbuseSounds",
        "TODO-Set-Copyright": "Microsoft and DontAbuseSounds contributors",
        "Internal Microphone Array - Front": "DontAbuseSounds Mic",
        "Simple Audio Sample Topology Microphone Array - Front": "DontAbuseSounds Mic",
        "Simple Audio Sample Wave Microphone Array - Front": "DontAbuseSounds Mic",
        "Simple Audio Sample Wave Speaker": "DontAbuseSounds Feed",
        "Simple Audio Sample Topology Speaker": "DontAbuseSounds Feed",
    }.items():
        text = replace_once(text, old, new)
    # Keep neither virtual endpoint eligible as the automatic OS default.
    line = r"HKR,EP\0,%PKEY_AudioEndpoint_Supports_EventDriven_Mode%,0x00010001,0x1"
    assert text.count(line) == 2
    text = text.replace(line, line + "\n" + r"HKR,EP\0,%PKEY_AudioDevice_NeverSetAsDefaultEndpoint%,0x00010001,0x307")
    text = replace_once(text, "[Strings]", '[Strings]\nPKEY_AudioDevice_NeverSetAsDefaultEndpoint = "{F3E80BEF-1723-4FF2-BCC4-7F83DC5E46D4},3"')
    return text


edit(main / "SimpleAudioSample.inx", inf, "utf-16")


def stream(text):
    # No sample tone, and no debug audio recordings.
    start = text.index("    if (m_bCapture)\n    {\n        ReadRegistrySettings();")
    end = text.index("    // Register this stream.", start)
    text = text[:start] + text[end:]
    text = replace_once(text, """        if (!g_DoNotCreateDataFiles)
        {
            // Read from buffer and write to a file.
            ReadBytes(ByteDisplacement);
        }""", "        ReadBytes(ByteDisplacement);")
    text = replace_once(text, "m_ToneGenerator.GenerateSine(m_pDmaBuffer + bufferOffset, runWrite);",
                        "ReadFeed(m_pDmaBuffer + bufferOffset, runWrite);")
    text = replace_once(text, "m_SaveData.WriteData(m_pDmaBuffer + bufferOffset, runWrite);",
                        "WriteFeed(m_pDmaBuffer + bufferOffset, runWrite);")
    for name in ("WriteBytes", "ReadBytes"):
        start = text.index("    ULONG bufferOffset", text.index("VOID CMiniportWaveRTStream::" + name))
        text = text[:start] + """    if (!m_pDmaBuffer || !m_ulDmaBufferSize)
        return;
    // A stalled client can only contribute its most recent DMA buffer.
    const ULONG skipped = ByteDisplacement > m_ulDmaBufferSize ? ByteDisplacement - m_ulDmaBufferSize : 0;
    ByteDisplacement -= skipped;
"""+text[start:].replace("m_ullLinearPosition % m_ulDmaBufferSize", "(m_ullLinearPosition + skipped) % m_ulDmaBufferSize", 1)
    text = replace_once(text, """            // Wait until all work items are completed.
            if (!m_bCapture && !g_DoNotCreateDataFiles)
            {
                m_SaveData.WaitAllWorkItems();
            }""", """            if (!m_bCapture)
                ClearFeed();""")
    text = replace_once(text, """            GetPositions(NULL, NULL, NULL);
            break;""", """            GetPositions(NULL, NULL, NULL);
            if (!m_bCapture)
                ClearFeed();
            break;""")
    text = replace_once(text, "m_SaveData.Disable(drmRights->CopyProtect);", """if (!m_bCapture) {
        KIRQL level;
        KeAcquireSpinLock(&gFeedLock, &level);
        gProtected = drmRights->CopyProtect;
        gFeedRead = gFeedWrite = gFeedBytes = 0;
        gPrimed = false;
        KeReleaseSpinLock(&gFeedLock, level);
    }""")
    mark = "//=============================================================================\n#pragma code_seg()\nVOID CMiniportWaveRTStream::WriteBytes"
    return replace_once(text, mark, LOOPBACK + "\n" + mark)


LOOPBACK = """// One render pin and one capture pin negotiate the same 48 kHz stereo PCM16.
#pragma code_seg()
namespace {
KSPIN_LOCK gFeedLock = 0;
BYTE gFeed[11520]; // 60 ms of PCM16 stereo; drop old samples when no client listens.
ULONG gFeedRead, gFeedWrite, gFeedBytes;
bool gProtected, gPrimed;

void ClearFeed() {
    KIRQL level;
    KeAcquireSpinLock(&gFeedLock, &level);
    gFeedRead = gFeedWrite = gFeedBytes = 0;
    gPrimed = false;
    KeReleaseSpinLock(&gFeedLock, level);
}

void WriteFeed(const BYTE* data, ULONG count) {
    constexpr ULONG capacity = sizeof(gFeed);
    if (count > capacity) {
        data += count - capacity;
        count = capacity;
    }
    KIRQL level;
    KeAcquireSpinLock(&gFeedLock, &level);
    if (!gProtected) {
        const ULONG first = min(count, capacity - gFeedWrite);
        RtlCopyMemory(gFeed + gFeedWrite, data, first);
        RtlCopyMemory(gFeed, data + first, count - first);
        gFeedWrite = (gFeedWrite + count) % capacity;
        gFeedBytes += count;
        if (gFeedBytes > capacity) {
            gFeedRead = (gFeedRead + gFeedBytes - capacity) % capacity;
            gFeedBytes = capacity;
        }
    }
    KeReleaseSpinLock(&gFeedLock, level);
}

void ReadFeed(BYTE* data, ULONG count) {
    constexpr ULONG capacity = sizeof(gFeed);
    KIRQL level;
    KeAcquireSpinLock(&gFeedLock, &level);
    if (gFeedBytes >= 3840)
        gPrimed = true;
    const ULONG valid = gPrimed ? min(count, gFeedBytes) : 0;
    const ULONG first = min(valid, capacity - gFeedRead);
    RtlCopyMemory(data, gFeed + gFeedRead, first);
    RtlCopyMemory(data + first, gFeed, valid - first);
    gFeedRead = (gFeedRead + valid) % capacity;
    gFeedBytes -= valid;
    if (valid < count)
        gPrimed = false;
    KeReleaseSpinLock(&gFeedLock, level);
    RtlZeroMemory(data + valid, count - valid);
}
}
"""
edit(main / "minwavertstream.cpp", lambda text: replace_once(
    stream(text), "NTSTATUS CMiniportWaveRTStream::SetState",
    "namespace { void ClearFeed(); }\n\nNTSTATUS CMiniportWaveRTStream::SetState",
))
