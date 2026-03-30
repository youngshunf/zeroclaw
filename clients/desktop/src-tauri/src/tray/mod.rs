pub mod events;
pub mod menu;

use tauri::{
    AppHandle, Manager, Runtime,
    image::Image,
    tray::{TrayIcon, TrayIconBuilder, TrayIconEvent, MouseButton},
};

// 预加载生成的机器人头像
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../../icons/tray_robot_icon.png");

pub fn setup_tray<R: Runtime>(app: &AppHandle<R>) -> Result<TrayIcon<R>, tauri::Error> {
    let menu = menu::create_tray_menu(app)?;

    let icon = Image::from_bytes(TRAY_ICON_BYTES)
        .expect("Tray icon invalid bytes");

    TrayIconBuilder::with_id("main")
        .tooltip("唤星 AI")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(events::handle_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button, .. } = event {
                if button == MouseButton::Left {
                    let app = tray.app_handle();
                    
                    // 托盘左键点击：若窗口显示且在最前，则隐藏；否则显示
                    if let Some(window) = app.get_webview_window("main") {
                        let is_visible = window.is_visible().unwrap_or(false);
                        let is_focused = window.is_focused().unwrap_or(false);
                        
                        // 简单处理：如果当前未显示则显示，反之亦然。实际可能会做更高阶的焦点判断。
                        if !is_visible {
                            events::show_main_window(app, None);
                        } else {
                            // 如果显示并且有焦点，点击托盘可收起窗口（视系统习惯，这里选择收起）
                            if is_focused {
                                let _ = window.hide();
                            } else {
                                events::show_main_window(app, None);
                            }
                        }
                    }
                }
            }
        })
        .build(app)
}
