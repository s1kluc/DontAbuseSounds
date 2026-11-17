#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod audio;
#[cfg(windows)]
mod capture_feed;
#[cfg(windows)]
mod capture_setup;
mod clip;
mod project;

use audio::{Engine, PadCommand, PADS};
use project::{Project, Settings};
use serde::Serialize;
use std::{path::PathBuf, sync::Mutex};
use tauri::{Manager, State};

struct Desktop {
    project: Project,
    engine: Option<Route>,
    generation: u64,
    pad_revisions: [u64; PADS],
}
enum Route {
    Device(Engine),
    #[cfg(windows)]
    Capture(capture_feed::CaptureFeed),
}

impl Route {
    fn open(input: Option<&str>, output: &str) -> Result<Self, String> {
        #[cfg(windows)]
        if output == capture_feed::ID {
            return capture_feed::CaptureFeed::open().map(Self::Capture);
        }
        Engine::open(input, output).map(Self::Device)
    }
    fn rate(&self) -> u32 {
        match self {
            Self::Device(engine) => engine.rate,
            #[cfg(windows)]
            Self::Capture(_) => capture_feed::RATE,
        }
    }
    fn status(&self) -> Option<u16> {
        match self {
            Self::Device(engine) => engine.status(),
            #[cfg(windows)]
            Self::Capture(feed) => feed.status(),
        }
    }
    fn send(&mut self, cmd: PadCommand) -> Result<(), String> {
        match self {
            Self::Device(engine) => engine.send(cmd),
            #[cfg(windows)]
            Self::Capture(feed) => feed.send(cmd),
        }
    }
}

type App<'a> = State<'a, Mutex<Desktop>>;

#[derive(Serialize)]
struct Startup {
    settings: Settings,
    load_error: Option<String>,
}

#[tauri::command]
fn settings(app: App<'_>) -> Startup {
    let app = app.lock().unwrap();
    Startup {
        settings: app.project.settings.clone(),
        load_error: app.project.load_error.clone(),
    }
}

#[tauri::command]
fn devices(input: bool) -> Result<Vec<audio::Endpoint>, String> {
    let devices = audio::devices(input)?;
    #[cfg(windows)]
    if !input && capture_feed::configured() {
        let mut devices = devices;
        devices.push(audio::Endpoint::capture_route());
        return Ok(devices);
    }
    Ok(devices)
}

#[tauri::command]
fn capture_setup_available() -> bool {
    #[cfg(windows)]
    {
        capture_setup::available()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[tauri::command]
async fn setup_capture(input: String) -> Result<bool, String> {
    #[cfg(windows)]
    {
        tauri::async_runtime::spawn_blocking(move || capture_setup::install(&input))
            .await
            .map_err(|e| e.to_string())?
    }
    #[cfg(not(windows))]
    {
        let _ = input;
        Err("Hardware microphone routing is available on Windows".into())
    }
}

#[tauri::command]
fn start_audio(app: App<'_>, input: Option<String>, output: String) -> Result<(), String> {
    let mut app = app.lock().unwrap();
    app.engine.take();
    app.generation += 1;
    let engine = Route::open(input.as_deref(), &output)?;
    if app.project.load_error.is_none() {
        app.project.set_devices(input, output)?;
    }
    app.engine = Some(engine);
    Ok(())
}

#[tauri::command]
fn stop_audio(app: App<'_>) {
    let mut app = app.lock().unwrap();
    app.engine.take();
    app.generation += 1;
}

#[tauri::command]
fn audio_status(app: App<'_>) -> Result<Option<u16>, String> {
    let mut app = app.lock().unwrap();
    match app.engine.as_ref().map(Route::status) {
        Some(None) => {
            app.engine.take();
            app.generation += 1;
            Err("An audio device stopped. Check your devices and start audio again.".into())
        }
        Some(Some(bits)) => Ok(Some(bits)),
        None => Ok(None),
    }
}

#[tauri::command]
async fn assign_sound(app: App<'_>, pad: usize, path: PathBuf) -> Result<Settings, String> {
    let path = tauri::async_runtime::spawn_blocking(move || {
        clip::decode(&path)?;
        Ok::<_, String>(path)
    })
    .await
    .map_err(|e| e.to_string())??;
    let mut app = app.lock().unwrap();
    app.project.assign(pad, &path)?;
    Ok(app.project.settings.clone())
}

#[tauri::command]
async fn import_project(handle: tauri::AppHandle, path: PathBuf) -> Result<Settings, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = handle.state::<Mutex<Desktop>>();
        let mut app = state.lock().unwrap();
        app.project.import(&path)?;
        Ok(app.project.settings.clone())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn play_pad(app: App<'_>, pad: usize) -> Result<(), String> {
    let (path, rate, generation, revision) = {
        let app = app.lock().unwrap();
        let engine = app
            .engine
            .as_ref()
            .ok_or("Select an output and start audio first")?;
        (
            app.project.path(pad)?,
            engine.rate(),
            app.generation,
            app.pad_revisions[pad],
        )
    };
    let samples = tauri::async_runtime::spawn_blocking(move || {
        let (frames, input_rate) = clip::decode(&path)?;
        clip::convert(frames, input_rate, rate)
    })
    .await
    .map_err(|e| e.to_string())??;
    let mut app = app.lock().unwrap();
    if generation != app.generation || revision != app.pad_revisions[pad] {
        return Err("Playback was cancelled".into());
    }
    app.engine
        .as_mut()
        .ok_or("Audio is stopped")?
        .send(PadCommand::Play(pad, samples))
}

#[tauri::command]
fn stop_pad(app: App<'_>, pad: Option<usize>) -> Result<(), String> {
    let mut app = app.lock().unwrap();
    if pad.is_some_and(|i| i >= PADS) {
        return Err("Invalid pad".into());
    }
    if let Some(i) = pad {
        app.pad_revisions[i] += 1;
    } else {
        app.generation += 1;
    }
    if let Some(engine) = app.engine.as_mut() {
        engine.send(pad.map_or(PadCommand::StopAll, PadCommand::Stop))?;
    }
    Ok(())
}

fn main() {
    #[cfg(windows)]
    capture_setup::run_setup_argument();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let root = app.path().app_local_data_dir()?;
            app.manage(Mutex::new(Desktop {
                project: Project::open(root),
                engine: None,
                generation: 0,
                pad_revisions: [0; PADS],
            }));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            settings,
            devices,
            capture_setup_available,
            setup_capture,
            start_audio,
            stop_audio,
            audio_status,
            assign_sound,
            import_project,
            play_pad,
            stop_pad
        ])
        .run(tauri::generate_context!())
        .expect("Could not start DontAbuseSounds");
}
