#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod audio;
mod clip;
mod project;

use audio::{Engine, PadCommand, PADS};
use project::{Project, Settings};
use serde::Serialize;
use std::{path::PathBuf, sync::Mutex};
use tauri::{Manager, State};

struct Desktop {
    project: Project,
    engine: Option<Engine>,
    generation: u64,
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
    audio::devices(input)
}

#[tauri::command]
fn start_audio(app: App<'_>, input: Option<String>, output: String) -> Result<(), String> {
    let mut app = app.lock().unwrap();
    app.engine.take();
    app.generation += 1;
    let engine = Engine::open(input.as_deref(), &output)?;
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
    match app.engine.as_ref().map(Engine::status) {
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
    let (path, rate, generation) = {
        let app = app.lock().unwrap();
        let engine = app
            .engine
            .as_ref()
            .ok_or("Select an output and start audio first")?;
        (app.project.path(pad)?, engine.rate, app.generation)
    };
    let samples = tauri::async_runtime::spawn_blocking(move || {
        let (frames, input_rate) = clip::decode(&path)?;
        clip::convert(frames, input_rate, rate)
    })
    .await
    .map_err(|e| e.to_string())??;
    let app = app.lock().unwrap();
    if generation != app.generation {
        return Err("Playback was cancelled".into());
    }
    app.engine
        .as_ref()
        .ok_or("Audio is stopped")?
        .send(PadCommand::Play(pad, samples))
}

#[tauri::command]
fn stop_pad(app: App<'_>, pad: Option<usize>) -> Result<(), String> {
    let mut app = app.lock().unwrap();
    if pad.is_some_and(|i| i >= PADS) {
        return Err("Invalid pad".into());
    }
    app.generation += 1;
    if let Some(engine) = app.engine.as_ref() {
        engine.send(pad.map_or(PadCommand::StopAll, PadCommand::Stop))?;
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let root = app.path().app_local_data_dir()?;
            app.manage(Mutex::new(Desktop {
                project: Project::open(root),
                engine: None,
                generation: 0,
            }));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            settings,
            devices,
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
