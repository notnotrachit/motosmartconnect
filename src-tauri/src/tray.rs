use tauri::{tray::{MouseButton, MouseButtonState, TrayIconEvent}, Manager, WebviewUrl, WebviewWindowBuilder};

const PANEL_WIDTH: f64 = 380.0;
const PANEL_HEIGHT: f64 = 520.0;

/// Creates a small, app-styled panel that is shown from the menu bar icon.
pub fn install<R: tauri::Runtime>(app: &tauri::App<R>) -> tauri::Result<()> {
    let panel = WebviewWindowBuilder::new(app, "tray-panel", WebviewUrl::App("menu.html".into()))
        .title("Smart Connect")
        .inner_size(PANEL_WIDTH, PANEL_HEIGHT)
        .min_inner_size(PANEL_WIDTH, PANEL_HEIGHT)
        .max_inner_size(PANEL_WIDTH, PANEL_HEIGHT)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .shadow(true)
        .build()?;
    let panel_for_blur = panel.clone();
    panel.on_window_event(move |event| {
        if let tauri::WindowEvent::Focused(false) = event {
            let _ = panel_for_blur.hide();
        }
    });

    let tray_id = String::from("smart-connect-tray");
    let tray = app
        .tray_by_id(&tray_id)
        .expect("configured Smart Connect menu bar icon");
    tray.on_tray_icon_event(move |tray, event| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            rect,
            ..
        } = event
        {
            let panel = tray.app_handle().get_webview_window("tray-panel");
            if let Some(panel) = panel {
                if panel.is_visible().unwrap_or(false) {
                    let _ = panel.hide();
                } else {
                    // Anchor the panel below the clicked menu-bar icon.
                    let scale = panel.scale_factor().unwrap_or(1.0);
                    let position = rect.position.to_physical::<f64>(scale);
                    let size = rect.size.to_physical::<u32>(scale);
                    let x = (position.x + f64::from(size.width) - PANEL_WIDTH * scale).max(8.0);
                    let y = position.y + f64::from(size.height) + 8.0;
                    let _ = panel.set_position(tauri::PhysicalPosition::new(x, y));
                    let _ = panel.show();
                    let _ = panel.set_focus();
                }
            }
        }
    });
    Ok(())
}
