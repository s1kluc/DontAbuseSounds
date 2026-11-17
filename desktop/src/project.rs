use crate::{audio::PADS, clip};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Pad {
    pub name: String,
    file: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    version: u8,
    pub pads: [Option<Pad>; PADS],
    pub input_id: Option<String>,
    pub output_id: Option<String>,
}

pub struct Project {
    root: PathBuf,
    pub settings: Settings,
    pub load_error: Option<String>,
}

impl Project {
    pub fn open(root: PathBuf) -> Self {
        let mut project = Self {
            root,
            settings: Settings {
                version: 2,
                pads: Default::default(),
                input_id: None,
                output_id: None,
            },
            load_error: None,
        };
        let file = project.root.join("project.json");
        match fs::read(&file) {
            Ok(data) => match parse(&data, 2) {
                Ok(s) => project.settings = s,
                Err(e) => project.load_error = Some(e),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => project.load_error = Some(e.to_string()),
        }
        project
    }

    pub fn path(&self, pad: usize) -> Result<PathBuf, String> {
        let pad = self
            .settings
            .pads
            .get(pad)
            .and_then(Option::as_ref)
            .ok_or("No sound is assigned")?;
        Ok(self.root.join("sounds").join(&pad.file))
    }

    fn writable(&self) -> Result<(), String> {
        if self.load_error.is_some() {
            return Err("Project is damaged. The original file was preserved".into());
        }
        Ok(())
    }

    pub fn set_devices(&mut self, input: Option<String>, output: String) -> Result<(), String> {
        let mut next = self.settings.clone();
        next.input_id = input;
        next.output_id = Some(output);
        self.save(next)
    }

    pub fn assign(&mut self, index: usize, path: &Path) -> Result<(), String> {
        self.writable()?;
        if index >= PADS {
            return Err("Invalid pad".into());
        }
        let name = path
            .file_stem()
            .ok_or("Invalid filename")?
            .to_string_lossy()
            .into_owned();
        let file = self.copy_sound(path)?;
        let mut next = self.settings.clone();
        let previous = next.pads[index].replace(Pad {
            name,
            file: file.clone(),
        });
        if let Err(err) = self.save(next) {
            let _ = fs::remove_file(self.root.join("sounds").join(file));
            return Err(err);
        }
        if let Some(old) = previous {
            if self
                .settings
                .pads
                .iter()
                .flatten()
                .all(|p| p.file != old.file)
            {
                let _ = fs::remove_file(self.root.join("sounds").join(old.file));
            }
        }
        Ok(())
    }

    pub fn import(&mut self, path: &Path) -> Result<(), String> {
        self.writable()?;
        if self.settings.pads.iter().any(Option::is_some) {
            return Err("Import an old project before assigning new sounds".into());
        }
        let bytes = fs::read(path).map_err(display)?;
        let mut previous = parse(&bytes, 1)?;
        let source = path
            .parent()
            .ok_or("Project directory missing")?
            .join("sounds");
        let mut copied = Vec::new();
        let result = (|| {
            for pad in previous.pads.iter_mut().flatten() {
                let path = source.join(&pad.file);
                if !fs::symlink_metadata(&path)
                    .map_err(display)?
                    .file_type()
                    .is_file()
                {
                    return Err("Project sound must be a regular file".into());
                }
                clip::decode(&path)?;
                pad.file = self.copy_sound(&path)?;
                copied.push(pad.file.clone());
            }
            previous.version = 2;
            previous.input_id = None;
            previous.output_id = None;
            self.save(previous)
        })();
        if result.is_err() {
            for file in copied {
                let _ = fs::remove_file(self.root.join("sounds").join(file));
            }
        }
        result
    }

    fn copy_sound(&self, path: &Path) -> Result<String, String> {
        let dir = self.root.join("sounds");
        fs::create_dir_all(&dir).map_err(display)?;
        let mut source = fs::File::open(path).map_err(display)?;
        let mut dest = tempfile::Builder::new()
            .prefix("sound-")
            .rand_bytes(20)
            .tempfile_in(dir)
            .map_err(display)?;
        std::io::copy(&mut source, &mut dest).map_err(display)?;
        dest.as_file().sync_all().map_err(display)?;
        let file = dest
            .path()
            .file_name()
            .ok_or("Invalid destination")?
            .to_string_lossy()
            .into_owned();
        dest.keep().map_err(display)?;
        Ok(file)
    }

    fn save(&mut self, next: Settings) -> Result<(), String> {
        self.writable()?;
        fs::create_dir_all(&self.root).map_err(display)?;
        let mut temp = tempfile::NamedTempFile::new_in(&self.root).map_err(display)?;
        serde_json::to_writer_pretty(&mut temp, &next).map_err(display)?;
        temp.flush().map_err(display)?;
        temp.as_file().sync_all().map_err(display)?;
        temp.persist(self.root.join("project.json"))
            .map_err(display)?;
        self.settings = next;
        Ok(())
    }
}

fn display(err: impl std::fmt::Display) -> String {
    err.to_string()
}

fn parse(bytes: &[u8], version: u8) -> Result<Settings, String> {
    let settings: Settings = serde_json::from_slice(bytes).map_err(display)?;
    if settings.version != version
        || settings.pads.iter().flatten().any(|p| {
            let name = &p.file;
            name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\', ':', '\0'])
        })
    {
        return Err("Invalid project version or sound filename".into());
    }
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_existing_project_survives_a_failed_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("project.json");
        fs::write(&path, "{broken").unwrap();
        let mut project = Project::open(dir.path().to_owned());
        assert!(project.set_devices(None, "device".into()).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "{broken");
    }
}
