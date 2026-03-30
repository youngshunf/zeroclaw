use tauri::{AppHandle, Manager, Runtime, Emitter, menu::MenuEvent};

pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    match event.id().as_ref() {
        "show_chat" => show_main_window(app, Some("/chat")),
        "show_docs" => show_main_window(app, Some("/docs")),
        "quit" => {
            // 用户主动选择退出菜单，允许应用进程终止
            std::process::exit(0);
        }
        _ => {}
    }
}

pub fn show_main_window<R: Runtime>(app: &AppHandle<R>, navigate_to: Option<&str>) {
    // 假设窗口 label 为 "main"
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        
        // 借助 hash 路由或自定义事件通知前端跳转
        if let Some(path) = navigate_to {
            // 使用 tauri::Emitter 发送跳转事件，更加优雅
            let _ = window.emit("huanxing:navigate", path);
        }
    }
}
