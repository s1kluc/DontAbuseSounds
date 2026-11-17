use crate::{audio::PadCommand, clip::Frame};
use std::{
    io::{BufWriter, Write},
    path::PathBuf,
    time::{Duration, Instant},
};

pub const ID: &str = "das:windows-capture";
pub const RATE: u32 = 48_000;

fn directory() -> Option<PathBuf> {
    std::env::var_os("ProgramData")
        .map(|path| PathBuf::from(path).join("DontAbuseSounds Capture Feed"))
}

pub fn configured() -> bool {
    directory().is_some_and(|path| path.join("installed").is_file())
}

pub struct CaptureFeed {
    directory: PathBuf,
    playing: Option<(usize, Instant)>,
}

impl CaptureFeed {
    pub fn open() -> Result<Self, String> {
        let directory = directory()
            .filter(|path| path.join("installed").is_file())
            .ok_or("Set up Windows microphone routing in the app first")?;
        let feed = Self {
            directory,
            playing: None,
        };
        feed.publish(&[])?;
        Ok(feed)
    }

    pub fn send(&mut self, command: PadCommand) -> Result<(), String> {
        match command {
            PadCommand::Play(pad, frames) => {
                if frames.len() > (RATE * 60) as usize {
                    return Err("The Windows capture test supports clips up to one minute".into());
                }
                self.publish(&frames)?;
                self.playing = Some((
                    pad,
                    Instant::now() + Duration::from_secs_f64(frames.len() as f64 / f64::from(RATE)),
                ));
            }
            PadCommand::Stop(pad) if self.playing.is_some_and(|(index, _)| index == pad) => {
                self.publish(&[])?;
                self.playing = None;
            }
            PadCommand::Stop(_) => {}
            PadCommand::StopAll => {
                self.publish(&[])?;
                self.playing = None;
            }
        }
        Ok(())
    }

    pub fn status(&self) -> Option<u16> {
        self.directory.join("installed").is_file().then(|| {
            self.playing
                .filter(|(_, end)| *end > Instant::now())
                .map_or(0, |(pad, _)| 1 << pad)
        })
    }

    fn publish(&self, frames: &[Frame]) -> Result<(), String> {
        let mut temp = tempfile::NamedTempFile::new_in(&self.directory).map_err(error)?;
        {
            let mut writer = BufWriter::new(temp.as_file_mut());
            writer.write_all(b"DAS1").map_err(error)?;
            writer
                .write_all(&(frames.len() as u32).to_le_bytes())
                .map_err(error)?;
            for [left, right] in frames {
                writer
                    .write_all(&((left + right) * 0.5).to_le_bytes())
                    .map_err(error)?;
            }
            writer.flush().map_err(error)?;
        }
        temp.persist(self.directory.join("clip.pcm"))
            .map_err(error)?;
        Ok(())
    }
}

impl Drop for CaptureFeed {
    fn drop(&mut self) {
        let _ = self.publish(&[]);
    }
}

fn error(error: impl std::fmt::Display) -> String {
    format!("Could not send audio to the Windows capture effect: {error}")
}
