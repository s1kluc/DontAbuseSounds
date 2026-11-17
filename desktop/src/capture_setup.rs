use base64::{engine::general_purpose::STANDARD, Engine};
use std::{
    io::{self, Write},
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        process::CommandExt,
    },
    path::PathBuf,
    process::{Command, Stdio},
    ptr,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_CANCELLED, WAIT_OBJECT_0},
    System::{
        Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE},
        SystemInformation::GetSystemDirectoryW,
        Threading::{GetExitCodeProcess, WaitForSingleObject, CREATE_NO_WINDOW, INFINITE},
    },
    UI::{
        Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW},
        WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK, SW_HIDE},
    },
};

const DLL: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/capture-effect.dll"));
const SCRIPT: &str = include_str!("../../drivers/windows/capture-probe/CaptureSetup.ps1");
const ARG: &str = "--configure-capture";
const NEEDS_RESTART: i32 = 3010;

pub fn available() -> bool {
    !DLL.is_empty()
}

fn endpoint_id(id: &str) -> Option<&str> {
    let (prefix, guid) = id.rsplit_once('.')?;
    let hex = guid.strip_prefix('{')?.strip_suffix('}')?;
    if !prefix.eq_ignore_ascii_case("wasapi:{0.0.1.00000000}") || hex.len() != 36 {
        return None;
    }
    hex.bytes()
        .enumerate()
        .all(|(i, byte)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
        .then_some(guid)
}

pub fn install(id: &str) -> Result<bool, String> {
    if !available() {
        return Err("This build does not contain the Windows capture effect".into());
    }
    let guid = endpoint_id(id).ok_or("Select a Windows recording device")?;
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let file: Vec<u16> = executable
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let verb = wide("runas");
    let parameters = wide(&format!("{ARG} {guid}"));
    let mut exec = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: verb.as_ptr(),
        lpFile: file.as_ptr(),
        lpParameters: parameters.as_ptr(),
        nShow: SW_HIDE,
        ..Default::default()
    };
    unsafe {
        let com = CoInitializeEx(
            ptr::null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        );
        let opened = ShellExecuteExW(&mut exec);
        let error = io::Error::last_os_error();
        if com >= 0 {
            CoUninitialize();
        }
        if opened == 0 {
            return Err(if error.raw_os_error() == Some(ERROR_CANCELLED as i32) {
                "Microphone setup was cancelled".into()
            } else {
                error.to_string()
            });
        }
        if exec.hProcess.is_null() {
            return Err("Windows did not start microphone setup".into());
        }
        let mut code = 1;
        let ok = WaitForSingleObject(exec.hProcess, INFINITE) == WAIT_OBJECT_0
            && GetExitCodeProcess(exec.hProcess, &mut code) != 0;
        CloseHandle(exec.hProcess);
        match (ok, code as i32) {
            (true, 0) => Ok(false),
            (true, NEEDS_RESTART) => Ok(true),
            _ => {
                Err("Windows microphone setup failed. Check the setup message for details.".into())
            }
        }
    }
}

pub fn run_setup_argument() {
    let mut args = std::env::args_os().skip(1);
    if args.next().is_none_or(|arg| arg != ARG) {
        return;
    }
    let result = args
        .next()
        .and_then(|arg| arg.into_string().ok())
        .ok_or_else(|| "Missing recording endpoint".into())
        .and_then(|guid| {
            if args.next().is_some() {
                return Err("Unexpected setup arguments".into());
            }
            let id = format!("wasapi:{{0.0.1.00000000}}.{guid}");
            endpoint_id(&id).ok_or("Invalid recording endpoint")?;
            configure(&guid)
        });
    match result {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            unsafe {
                MessageBoxW(
                    ptr::null_mut(),
                    wide(&error).as_ptr(),
                    wide("DontAbuseSounds microphone setup").as_ptr(),
                    MB_OK | MB_ICONERROR,
                );
            }
            std::process::exit(1);
        }
    }
}

fn configure(guid: &str) -> Result<i32, String> {
    if !available() {
        return Err("Missing audio effect".into());
    }
    let system = system_directory().map_err(|e| e.to_string())?;
    let powershell = system.join(r"WindowsPowerShell\v1.0\powershell.exe");
    // The elevated executable owns both payloads. It never reads a script or DLL from the download folder.
    let script = format!("try {{ & {{\n{SCRIPT}\n}} -Action Install -Endpoint '{guid}' -EmbeddedDll }} catch {{ [Console]::Error.WriteLine($_.Exception.Message); exit 1 }}");
    let encoded = STANDARD.encode(
        script
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    let mut child = Command::new(powershell)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
            &encoded,
        ])
        .env(
            "PSModulePath",
            system.join(r"WindowsPowerShell\v1.0\Modules"),
        )
        .current_dir(&system)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let write = child
        .stdin
        .take()
        .ok_or("Could not deliver the audio effect")?
        .write_all(STANDARD.encode(DLL).as_bytes());
    let result = child.wait_with_output().map_err(|e| e.to_string())?;
    if !result.status.success() || write.is_err() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(if stderr.trim().is_empty() {
            "Could not install microphone routing".into()
        } else {
            stderr.trim().into()
        });
    }

    let instance = format!(r"SWD\MMDEVAPI\{{0.0.1.00000000}}.{guid}");
    let restarted = Command::new(system.join("pnputil.exe"))
        .args(["/restart-device", &instance])
        .current_dir(&system)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .is_ok_and(|result| result.status.success());
    Ok(if restarted { 0 } else { NEEDS_RESTART })
}

fn system_directory() -> io::Result<PathBuf> {
    let mut buffer = [0u16; 260];
    let used = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
    if used == 0 || used >= buffer.len() {
        return Err(io::Error::last_os_error());
    }
    Ok(PathBuf::from(std::ffi::OsString::from_wide(
        &buffer[..used],
    )))
}

fn wide(input: &str) -> Vec<u16> {
    input.encode_utf16().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_setup_can_start_on_windows_without_installing_an_effect() {
        if !available() {
            return;
        }
        let error = configure("invalid-id").unwrap_err();
        assert!(
            error.contains("Invalid Windows recording endpoint"),
            "{error}"
        );
    }
    #[test]
    fn setup_only_accepts_canonical_capture_ids() {
        let valid = "wasapi:{0.0.1.00000000}.{9ebd2575-1dda-431d-b952-0a7ea9479cc9}";
        assert_eq!(
            endpoint_id(valid),
            Some("{9ebd2575-1dda-431d-b952-0a7ea9479cc9}")
        );
        assert!(endpoint_id(&valid.replace("0.0.1", "0.0.0")).is_none());
        assert!(endpoint_id(&valid.replace("9ebd", "'bad")).is_none());
    }
}
