//! Shared Tauri bootstrap for desktop and mobile entry points.

mod session;

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use session::SessionBridge;
use tauri::ipc::Channel;
use tauri::{Manager, RunEvent, State};
use vactr::cli::args::HostChoice;
#[cfg(target_os = "ios")]
use vactr::cli::owner::AudioSessionEvent;

static IOS_AUDIO_BRIDGE: OnceLock<Mutex<Option<Weak<SessionBridge>>>> = OnceLock::new();

struct SessionState(Arc<SessionBridge>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let app_data_dir: PathBuf = app.path().app_data_dir()?;
            fs::create_dir_all(&app_data_dir)?;
            let bridge = Arc::new(SessionBridge::new(HostChoice::Native, app_data_dir));
            let audio_bridge = IOS_AUDIO_BRIDGE.get_or_init(|| Mutex::new(None));
            if let Ok(mut current) = audio_bridge.lock() {
                *current = Some(Arc::downgrade(&bridge));
            }
            app.manage(SessionState(bridge));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            session_connect,
            session_send,
            session_close,
            self_check_enabled,
            self_check
        ])
        .build(tauri::generate_context!())
        .expect("error while building the Vactr Tauri shell")
        .run(|app, event| {
            if matches!(event, RunEvent::Exit) {
                if let Some(state) = app.try_state::<SessionState>() {
                    let _ = state.0.shutdown();
                }
                if let Some(audio_bridge) = IOS_AUDIO_BRIDGE.get() {
                    if let Ok(mut current) = audio_bridge.lock() {
                        *current = None;
                    }
                }
            }
        });
}

#[tauri::command]
fn session_connect(
    channel: Channel<String>,
    state: State<'_, SessionState>,
) -> Result<u32, String> {
    let (outbound, incoming) = std::sync::mpsc::channel();
    let id = state.0.connect(outbound)?;
    let forwarding_thread = std::thread::spawn(move || {
        for text in incoming {
            if channel.send(text).is_err() {
                break;
            }
        }
    });
    state.0.retain_forwarder(forwarding_thread)?;
    Ok(id)
}

#[tauri::command]
fn session_send(id: u32, text: String, state: State<'_, SessionState>) -> Result<(), String> {
    state.0.send(id, text)
}

#[tauri::command]
fn session_close(id: u32, state: State<'_, SessionState>) -> Result<(), String> {
    state.0.close(id)
}

#[tauri::command]
fn self_check_enabled() -> bool {
    std::env::var("VACTR_SELF_CHECK").as_deref() == Ok("1")
}

#[tauri::command]
fn self_check(report: String) {
    eprintln!("VACTR_SELF_CHECK {report}");
}

#[cfg(target_os = "ios")]
#[no_mangle]
pub extern "C" fn vactr_audio_session_event(kind: u32) {
    let event = match kind {
        1 => AudioSessionEvent::Interrupted,
        2 => AudioSessionEvent::Resumed,
        3 => AudioSessionEvent::RouteChanged,
        _ => return,
    };
    let Some(audio_bridge) = IOS_AUDIO_BRIDGE.get() else {
        return;
    };
    let Ok(current) = audio_bridge.lock() else {
        return;
    };
    if let Some(bridge) = current.as_ref().and_then(Weak::upgrade) {
        let _ = bridge.audio_event(event);
    }
}
