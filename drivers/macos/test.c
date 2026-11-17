#include "Mic.c"
#include <assert.h>

int main(void) {
    AudioServerPlugInDriverRef driver = gAudioServerPlugInDriverRef;
    const UInt32 frames = 2;
    Float32 output[] = {0.25f, -0.5f, 0.75f, -1.0f};
    Float32 input[4];
    AudioServerPlugInIOCycleInfo cycle = {0};

    assert(NullAudio_StartIO(driver, kObjectID_Device, 1) == 0);
    cycle.mOutputTime.mSampleTime = kLoopbackFrames - 1;
    assert(NullAudio_DoIOOperation(driver, kObjectID_Device, kObjectID_Stream_Output, 1,
        kAudioServerPlugInIOOperationWriteMix, frames, &cycle, output, NULL) == 0);
    cycle.mInputTime.mSampleTime = cycle.mOutputTime.mSampleTime + kLoopbackDelay;
    for (UInt32 client = 1; client <= 2; ++client) {
        assert(NullAudio_DoIOOperation(driver, kObjectID_Device, kObjectID_Stream_Input, client,
            kAudioServerPlugInIOOperationReadInput, frames, &cycle, input, NULL) == 0);
        assert(memcmp(input, output, sizeof(input)) == 0);
    }
    cycle.mInputTime.mSampleTime += kLoopbackFrames;
    assert(NullAudio_DoIOOperation(driver, kObjectID_Device, kObjectID_Stream_Input, 1,
        kAudioServerPlugInIOOperationReadInput, frames, &cycle, input, NULL) == 0);
    for (UInt32 i = 0; i < 4; ++i) assert(input[i] == 0);
    assert(NullAudio_StopIO(driver, kObjectID_Device, 1) == 0);
    assert(NullAudio_StartIO(driver, kObjectID_Device, 1) == 0);
    cycle.mInputTime.mSampleTime -= kLoopbackFrames;
    assert(NullAudio_DoIOOperation(driver, kObjectID_Device, kObjectID_Stream_Input, 1,
        kAudioServerPlugInIOOperationReadInput, frames, &cycle, input, NULL) == 0);
    for (UInt32 i = 0; i < 4; ++i) assert(input[i] == 0);
    assert(NullAudio_StopIO(driver, kObjectID_Device, 1) == 0);
}
