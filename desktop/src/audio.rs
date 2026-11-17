use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, FromSample, Sample, SampleFormat, SizedSample, Stream, SupportedStreamConfig,
};
use ringbuf::{
    traits::{Consumer, Observer, Producer, Split},
    HeapRb,
};
use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, AtomicU16, Ordering::Relaxed},
    mpsc::{self, Receiver, SyncSender},
    Arc,
};

use crate::clip::Frame;
pub const PADS: usize = 12;

#[derive(Serialize)]
pub struct Endpoint {
    id: String,
    name: String,
    virtual_mic: bool,
}
pub fn devices(input: bool) -> Result<Vec<Endpoint>, String> {
    let host = cpal::default_host();
    let enumerate = || -> Result<Vec<Device>, cpal::DevicesError> {
        Ok(if input {
            host.input_devices()?.collect()
        } else {
            host.output_devices()?.collect()
        })
    };
    Ok(enumerate()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|dev| {
            let id = dev.id().ok()?.to_string();
            let name = dev.description().ok()?.name().to_owned();
            let virtual_mic = own_device(&name);
            if input && virtual_mic {
                return None;
            }
            Some(Endpoint {
                id,
                name,
                virtual_mic,
            })
        })
        .collect())
}

fn own_device(name: &str) -> bool {
    name.contains("DontAbuseSounds Mic")
        || name.contains("DontAbuseSounds Feed")
        || name.contains("DontAbuseSounds Virtual Audio")
}

pub enum PadCommand {
    Play(usize, Vec<Frame>),
    Stop(usize),
    StopAll,
}

pub struct Engine {
    _output: Stream,
    _input: Option<Stream>,
    tx: SyncSender<PadCommand>,
    status: Arc<AtomicU16>,
    failure: Arc<AtomicBool>,
    pub rate: u32,
}
type InputTap = Box<dyn FnMut() -> f32 + Send>;

impl Engine {
    pub fn open(input: Option<&str>, output: &str) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = |id: &str| {
            let parsed: cpal::DeviceId = id.parse().map_err(|_| "Invalid device ID")?;
            host.device_by_id(&parsed).ok_or("Device unavailable")
        };
        let output_dev = device(output)?;
        let output_cfg = output_dev
            .default_output_config()
            .map_err(|e| e.to_string())?;
        let rate = output_cfg.sample_rate();
        let failure = Arc::new(AtomicBool::new(false));
        let (input_stream, tap) = if let Some(id) = input {
            let input_dev = device(id)?;
            if input_dev
                .description()
                .map(|d| own_device(d.name()))
                .unwrap_or(false)
            {
                return Err("Choose a hardware microphone as input".into());
            }
            let cfg = input_dev
                .default_input_config()
                .map_err(|e| e.to_string())?;
            let (stream, read) = microphone(&input_dev, cfg, rate, failure.clone())?;
            (Some(stream), Some(read))
        } else {
            (None, None)
        };
        let (tx, rx) = mpsc::sync_channel(32);
        let status = Arc::new(AtomicU16::new(0));
        let writer = Output {
            commands: rx,
            voice: tap,
            mixer: Mixer::default(),
            status: status.clone(),
            failure: failure.clone(),
        };
        let output_stream = output_stream(&output_dev, output_cfg, writer)?;
        if let Some(stream) = &input_stream {
            stream.play().map_err(|e| e.to_string())?;
        }
        output_stream.play().map_err(|e| e.to_string())?;
        Ok(Self {
            _output: output_stream,
            _input: input_stream,
            tx,
            status,
            failure,
            rate,
        })
    }

    pub fn send(&self, cmd: PadCommand) -> Result<(), String> {
        self.tx
            .try_send(cmd)
            .map_err(|_| "Audio engine is busy or disconnected".into())
    }
    pub fn status(&self) -> Option<u16> {
        (!self.failure.load(Relaxed)).then(|| self.status.load(Relaxed))
    }
}

#[derive(Default)]
struct Mixer {
    pads: [Option<(Vec<Frame>, usize)>; PADS],
}
impl Mixer {
    fn command(&mut self, cmd: PadCommand) {
        match cmd {
            PadCommand::Play(i, clip) => self.pads[i] = Some((clip, 0)),
            PadCommand::Stop(i) => {
                if let Some((clip, n)) = &mut self.pads[i] {
                    *n = clip.len();
                }
            }
            PadCommand::StopAll => {
                for (clip, n) in self.pads.iter_mut().flatten() {
                    *n = clip.len();
                }
            }
        }
    }
    fn next(&mut self, mic: f32) -> Frame {
        let mut sum = [mic; 2];
        for (clip, n) in self.pads.iter_mut().flatten() {
            if let Some(frame) = clip.get(*n) {
                sum[0] += frame[0] * 0.75;
                sum[1] += frame[1] * 0.75;
                *n += 1;
            }
        }
        sum.map(|v| v.clamp(-1.0, 1.0))
    }
    fn playing(&self) -> u16 {
        self.pads.iter().enumerate().fold(0, |bits, (i, pad)| {
            bits | ((pad.as_ref().is_some_and(|(clip, n)| *n < clip.len()) as u16) << i)
        })
    }
}

fn microphone(
    dev: &Device,
    cfg: SupportedStreamConfig,
    output_rate: u32,
    fail: Arc<AtomicBool>,
) -> Result<(Stream, InputTap), String> {
    let input_rate = cfg.sample_rate();
    let target = (input_rate as usize / 30).max(2);
    let (mut writer, mut reader) = HeapRb::<f32>::new(target * 4).split();
    let write = move |x: f32| {
        let _ = writer.try_push(if x.is_finite() {
            x.clamp(-1.0, 1.0)
        } else {
            0.0
        });
    };
    let ratio = f64::from(input_rate) / f64::from(output_rate);
    let (mut a, mut b, mut phase, mut active) = (0.0_f32, 0.0_f32, 0.0_f64, false);
    // Keep the live buffer near 33 ms by adjusting the input/output clock ratio.
    let mut adjustment = 0.0;
    let read = move || {
        if !active {
            if reader.occupied_len() < target {
                return 0.0;
            }
            a = reader.try_pop().unwrap_or_default();
            b = reader.try_pop().unwrap_or_default();
            phase = 0.0;
            adjustment = 0.0;
            active = true;
        }
        let value = a + (b - a) * phase as f32;
        let error = reader.occupied_len() as f64 / target as f64 - 1.0;
        adjustment += (error.clamp(-1.0, 1.0) * 0.005 - adjustment) * 0.001;
        phase += ratio * (1.0 + adjustment);
        while phase >= 1.0 {
            phase -= 1.0;
            a = b;
            if let Some(sample) = reader.try_pop() {
                b = sample;
            } else {
                active = false;
                break;
            }
        }
        value
    };
    let stream = match cfg.sample_format() {
        SampleFormat::F32 => capture::<f32>(dev, &cfg, write, fail),
        SampleFormat::I16 => capture::<i16>(dev, &cfg, write, fail),
        SampleFormat::I32 => capture::<i32>(dev, &cfg, write, fail),
        SampleFormat::U16 => capture::<u16>(dev, &cfg, write, fail),
        other => return Err(format!("Unsupported microphone sample format: {other}")),
    }?;
    Ok((stream, Box::new(read)))
}

fn capture<T>(
    dev: &Device,
    cfg: &SupportedStreamConfig,
    mut write: impl FnMut(f32) + Send + 'static,
    fail: Arc<AtomicBool>,
) -> Result<Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = cfg.channels() as usize;
    dev.build_input_stream(
        &cfg.config(),
        move |data: &[T], _| {
            for f in data.chunks_exact(channels) {
                write(f32::from_sample(f[0]));
            }
        },
        move |_| fail.store(true, Relaxed),
        None,
    )
    .map_err(|e| e.to_string())
}

struct Output {
    commands: Receiver<PadCommand>,
    voice: Option<InputTap>,
    mixer: Mixer,
    status: Arc<AtomicU16>,
    failure: Arc<AtomicBool>,
}

fn output_stream(
    dev: &Device,
    cfg: SupportedStreamConfig,
    output: Output,
) -> Result<Stream, String> {
    match cfg.sample_format() {
        SampleFormat::F32 => playback::<f32>(dev, &cfg, output),
        SampleFormat::I16 => playback::<i16>(dev, &cfg, output),
        SampleFormat::I32 => playback::<i32>(dev, &cfg, output),
        SampleFormat::U16 => playback::<u16>(dev, &cfg, output),
        other => Err(format!("Unsupported output sample format: {other}")),
    }
}

fn playback<T>(
    dev: &Device,
    cfg: &SupportedStreamConfig,
    mut output: Output,
) -> Result<Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = cfg.channels() as usize;
    let fail = output.failure.clone();
    dev.build_output_stream(
        &cfg.config(),
        move |data: &mut [T], _| {
            if output.failure.load(Relaxed) {
                data.fill(T::from_sample(0.0));
                return;
            }
            for cmd in output.commands.try_iter().take(32) {
                output.mixer.command(cmd);
            }
            for f in data.chunks_exact_mut(channels) {
                let mic = output.voice.as_mut().map_or(0.0, |read| read());
                let [l, r] = output.mixer.next(mic);
                if channels == 1 {
                    f[0] = T::from_sample((l + r) * 0.5);
                } else {
                    f[0] = T::from_sample(l);
                    f[1] = T::from_sample(r);
                    f[2..].fill(T::from_sample(0.0));
                }
            }
            output.status.store(output.mixer.playing(), Relaxed);
        },
        move |_| fail.store(true, Relaxed),
        None,
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stop_all_preserves_voice_and_mix_is_bounded() {
        let mut mixer = Mixer::default();
        mixer.command(PadCommand::Play(0, vec![[1.0, -1.0]; 2]));
        mixer.command(PadCommand::Play(1, vec![[1.0, -1.0]; 2]));
        assert_eq!(mixer.next(0.25), [1.0, -1.0]);
        assert_eq!(mixer.playing(), 3);
        mixer.command(PadCommand::StopAll);
        assert_eq!(mixer.next(0.25), [0.25; 2]);
        assert_eq!(mixer.playing(), 0);
    }
}
