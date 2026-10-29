mod cam_publish;
mod desk;
mod engine;
mod power;
mod sms;
mod fex;
mod mirror;
mod nfs;
mod net;
mod protocol;
mod store;

use engine::{Engine, Snapshot};
use tauri::{window::Color, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};

#[tauri::command]
fn snapshot(state: State<'_, Engine>) -> Snapshot {
    state.snapshot()
}

#[tauri::command]
fn wallpaper(state: State<'_, Engine>) -> String {
    state.wallpaper()
}

#[tauri::command]
fn set_name(state: State<'_, Engine>, name: String) -> Result<Snapshot, String> {
    state.set_name(&name)?;
    Ok(state.snapshot())
}

#[tauri::command]
fn refresh_qr(state: State<'_, Engine>) -> Snapshot {
    state.refresh_qr();
    state.snapshot()
}

#[tauri::command]
fn begin_pairing(state: State<'_, Engine>) -> Snapshot {
    state.begin_pairing();
    state.snapshot()
}

#[tauri::command]
fn end_pairing(state: State<'_, Engine>) -> Snapshot {
    state.end_pairing();
    state.snapshot()
}

#[tauri::command]
async fn unpair(state: State<'_, Engine>, real_id: String) -> Result<Snapshot, String> {
    state.unpair(&real_id).await;
    Ok(state.snapshot())
}

#[tauri::command]
fn messages_open(state: State<'_, Engine>, key: String) -> Result<Snapshot, String> {
    state.messages_open(&key)?;
    Ok(state.snapshot())
}

#[tauri::command]
fn messages_close(state: State<'_, Engine>) -> Snapshot {
    state.messages_close();
    state.snapshot()
}

#[tauri::command]
fn messages_send(state: State<'_, Engine>, text: String) -> Result<Snapshot, String> {
    state.messages_send(&text)?;
    Ok(state.snapshot())
}

#[tauri::command]
fn messages_send_new(state: State<'_, Engine>, address: String, text: String) -> Result<Snapshot, String> {
    state.messages_send_new(&address, &text)?;
    Ok(state.snapshot())
}

#[tauri::command]
fn notification_reply(state: State<'_, Engine>, key: String, text: String) -> Result<Snapshot, String> {
    state.notification_reply(&key, &text)?;
    Ok(state.snapshot())
}

#[tauri::command]
fn notification_act(state: State<'_, Engine>, key: String, action_id: i64) -> Result<Snapshot, String> {
    state.notification_act(&key, action_id)?;
    Ok(state.snapshot())
}

#[tauri::command]
fn notification_open(state: State<'_, Engine>, key: String) -> Result<Snapshot, String> {
    state.notification_open(&key)?;
    Ok(state.snapshot())
}

#[tauri::command]
fn start_mirror(state: State<'_, Engine>) -> Result<Snapshot, String> {
    state.start_mirror()?;
    Ok(state.snapshot())
}

#[tauri::command]
fn start_desktop(state: State<'_, Engine>) -> Result<Snapshot, String> {
    state.start_desktop()?;
    Ok(state.snapshot())
}

#[tauri::command]
async fn list_phone(state: State<'_, Engine>, path: String) -> Result<Vec<nfs::PhoneEntry>, String> {
    state.list_phone(&path).await
}

#[tauri::command]
async fn save_phone(state: State<'_, Engine>, path: String) -> Result<String, String> {
    state.save_phone(&path).await
}

#[tauri::command]
fn start_app(state: State<'_, Engine>, pkg: String, cmp: String, user_id: i64) -> Result<Snapshot, String> {
    state.start_app(&pkg, &cmp, user_id)?;
    Ok(state.snapshot())
}

#[tauri::command]
fn start_webcam(state: State<'_, Engine>) -> Result<Snapshot, String> {
    state.start_webcam()?;
    Ok(state.snapshot())
}

#[tauri::command]
fn webcam_flip(state: State<'_, Engine>) -> Result<Snapshot, String> {
    state.webcam_flip()?;
    Ok(state.snapshot())
}

#[tauri::command]
fn open_stream_window(app: tauri::AppHandle, pkg: String, title: String) -> Result<(), String> {
    let pkg = pkg.trim().to_string();
    if pkg.is_empty() || pkg.len() > 200 || pkg.contains("..") || pkg.contains('/') || pkg.contains('\\') {
        return Err("That app cannot be opened.".into());
    }
    let label = stream_label(&pkg);
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }
    let wide = pkg == "desktop" || pkg == "webcam";
    let title: String = {
        let title = title.trim();
        if title.is_empty() {
            "Phone app".into()
        } else {
            title.chars().take(80).collect()
        }
    };
    let script = format!(
        "window.__STREAM_PKG__ = {};",
        serde_json::to_string(&pkg).unwrap_or_else(|_| "\"\"".into())
    );
    // The page must not resize this window after video starts. A resize drops the picture.
    let window = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App("stream.html".into()))
        .title(title)
        .inner_size(if wide { 1280.0 } else { 390.0 }, if wide { 800.0 } else { 844.0 })
        .min_inner_size(if wide { 960.0 } else { 280.0 }, if wide { 600.0 } else { 480.0 })
        .background_color(Color(0, 0, 0, 255))
        .initialization_script(&script)
        .build()
        .map_err(|err| err.to_string())?;
    // A page listener makes Tauri swallow the red button until the page closes the window itself.
    let engine = app.state::<Engine>().inner().clone();
    let watched = pkg;
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { .. } = event {
            engine.stop_pkg(&watched);
        }
    });
    Ok(())
}

#[tauri::command]
fn stop_stream(state: State<'_, Engine>, session_id: String) -> Result<Snapshot, String> {
    state.stop_stream(&session_id);
    Ok(state.snapshot())
}

#[tauri::command]
fn play_decoded(state: State<'_, Engine>, session_id: String, sdp: String) -> Result<(), String> {
    state.play_decoded(&session_id, &sdp)
}

#[tauri::command]
fn page_offers(state: State<'_, Engine>) -> Vec<serde_json::Value> {
    state.page_offers()
}

#[tauri::command]
fn webrtc_answer(state: State<'_, Engine>, session_id: String, sdp: String) -> Result<(), String> {
    state.webrtc_answer(&session_id, &sdp)
}

#[tauri::command]
fn webrtc_ice(state: State<'_, Engine>, session_id: String, candidate: String, label: i64, mid: String) {
    state.webrtc_ice(&session_id, &candidate, label, &mid);
}

#[tauri::command]
fn screen_note(state: State<'_, Engine>, text: String) {
    state.screen_note(&text);
}

#[tauri::command]
fn mirror_frame(state: State<'_, Engine>, since: u64) -> mirror::MirrorView {
    state.mirror_view(since)
}

#[tauri::command]
fn cross_start(state: State<'_, Engine>) -> Result<Snapshot, String> {
    state.cross_start()?;
    Ok(state.snapshot())
}

#[tauri::command]
fn cross_stop(state: State<'_, Engine>) -> Snapshot {
    state.cross_stop();
    state.snapshot()
}

#[tauri::command]
fn cross_show(state: State<'_, Engine>, show: bool) {
    state.cross_show(show);
}

#[tauri::command]
fn send_input(state: State<'_, Engine>, display_id: i64, kind: String, x: i64, y: i64, key: i64, down: bool, flags: i64, text: String) {
    state.send_input(display_id, &kind, x, y, key, down, flags, &text);
}

#[tauri::command]
fn refresh_apps(state: State<'_, Engine>) -> Result<Snapshot, String> {
    state.refresh_apps()?;
    Ok(state.snapshot())
}

#[tauri::command]
fn app_icons(state: State<'_, Engine>) -> engine::IconPack {
    state.app_icons()
}

#[tauri::command]
fn reveal_file(state: State<'_, Engine>, path: String) -> Result<(), String> {
    state.reveal_file(&path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let engine = Engine::new(app.handle().clone()).map_err(|err| -> Box<dyn std::error::Error> {
                Box::new(std::io::Error::other(format!("{err:#}")))
            })?;
            app.manage(engine.clone());
            tauri::async_runtime::spawn(async move {
                engine.boot().await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            wallpaper,
            set_name,
            refresh_qr,
            begin_pairing,
            end_pairing,
            unpair,
            messages_open,
            messages_close,
            messages_send,
            messages_send_new,
            notification_reply,
            notification_act,
            notification_open,
            start_mirror,
            start_desktop,
            start_app,
            start_webcam,
            webcam_flip,
            open_stream_window,
            stop_stream,
            play_decoded,
            page_offers,
            webrtc_answer,
            webrtc_ice,
            screen_note,
            mirror_frame,
            cross_start,
            cross_stop,
            cross_show,
            send_input,
            refresh_apps,
            app_icons,
            reveal_file,
            list_phone,
            save_phone
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn stream_label(pkg: &str) -> String {
    let mut label = String::from("stream-");
    for ch in pkg.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            label.push(ch);
        } else {
            label.push('_');
        }
        if label.len() >= 60 {
            break;
        }
    }
    label
}


