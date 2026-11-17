use std::{fs::File, path::Path};

use rubato::{FftFixedInOut, Resampler};
use symphonia::core::{
    formats::{probe::Hint, TrackType},
    io::MediaSourceStream,
};

pub type Frame = [f32; 2];

pub fn decode(path: &Path) -> Result<(Vec<Frame>, u32), String> {
    decode_inner(path).map_err(|err| format!("Can't open audio: {err}"))
}

fn decode_inner(path: &Path) -> Result<(Vec<Frame>, u32), Box<dyn std::error::Error>> {
    let source = MediaSourceStream::new(Box::new(File::open(path)?), Default::default());
    let mut format = symphonia::default::get_probe().probe(
        &Hint::new(),
        source,
        Default::default(),
        Default::default(),
    )?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or("No audio track")?;
    let id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or("Unknown audio format")?;
    let mut decoder =
        symphonia::default::get_codecs().make_audio_decoder(params, &Default::default())?;
    let mut output = Vec::new();
    let mut interleaved = Vec::new();
    let mut rate = 0;
    while let Some(packet) = format.next_packet()? {
        if packet.track_id != id {
            continue;
        }
        let decoded = decoder.decode(&packet)?;
        let spec = decoded.spec();
        let channels = spec.channels().count();
        let packet_rate = spec.rate();
        if !(1..=2).contains(&channels)
            || !(8000..=192000).contains(&packet_rate)
            || (rate != 0 && rate != packet_rate)
        {
            return Err(
                "Choose mono or stereo audio with a constant sample rate between 8 and 192 kHz"
                    .into(),
            );
        }
        rate = packet_rate;
        let frames = output.len() + decoded.samples_interleaved() / channels;
        if frames > rate as usize * 300 || frames * 2 > 30_000_000 {
            return Err("Choose audio up to five minutes and 30 million stereo samples".into());
        }
        interleaved.resize(decoded.samples_interleaved(), 0.0_f32);
        decoded.copy_to_slice_interleaved(&mut interleaved);
        output.extend(interleaved.chunks_exact(channels).map(|f| {
            let clean = |x: f32| {
                if x.is_finite() {
                    x.clamp(-1.0, 1.0)
                } else {
                    0.0
                }
            };
            [clean(f[0]), clean(f[channels - 1])]
        }));
    }
    if output.is_empty() {
        return Err("Empty audio file".into());
    }
    Ok((output, rate))
}

pub fn convert(frames: Vec<Frame>, rate: u32, output_rate: u32) -> Result<Vec<Frame>, String> {
    if rate == output_rate {
        return Ok(frames);
    }
    if !(8000..=192000).contains(&output_rate) {
        return Err("Unsupported output sample rate".into());
    }
    let mut converter = FftFixedInOut::<f32>::new(rate as usize, output_rate as usize, 1024, 2)
        .map_err(|e| e.to_string())?;
    let frames_out =
        (frames.len() as u64 * u64::from(output_rate)).div_ceil(u64::from(rate)) as usize;
    if frames_out > 15_000_000 {
        return Err("Clip is too long at the selected output's sample rate".into());
    }
    let mut input = converter.input_buffer_allocate(true);
    let mut output = converter.output_buffer_allocate(true);
    let mut result = Vec::with_capacity(frames_out);
    let mut delay = converter.output_delay();
    let mut index = 0;
    while result.len() < frames_out {
        let next = converter.input_frames_next();
        for j in 0..next {
            let f = frames.get(index + j).copied().unwrap_or_default();
            input[0][j] = f[0];
            input[1][j] = f[1];
        }
        index += next;
        let (_, written) = converter
            .process_into_buffer(&input, &mut output, None)
            .map_err(|e| e.to_string())?;
        let skip = delay.min(written);
        delay -= skip;
        let count = (written - skip).min(frames_out - result.len());
        result.extend((skip..skip + count).map(|i| [output[0][i], output[1][i]]));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conversion_keeps_length_and_channels() {
        let result = convert(vec![[0.5, -0.25]; 4410], 44100, 48000).unwrap();
        assert_eq!(result.len(), 4800);
        assert!((result[1000][0] - 0.5).abs() < 0.01);
        assert!((result[1000][1] + 0.25).abs() < 0.01);
    }
}
