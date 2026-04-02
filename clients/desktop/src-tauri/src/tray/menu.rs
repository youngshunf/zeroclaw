use tauri::{
    menu::{Menu, MenuItemBuilder, PredefinedMenuItem},
    AppHandle, Runtime,
};

pub fn create_tray_menu<R: Runtime>(app: &AppHandle<R>) -> Result<Menu<R>, tauri::Error> {
    let show_chat = MenuItemBuilder::with_id("show_chat", "💻 打开会话列表").build(app)?;
    let show_docs = MenuItemBuilder::with_id("show_docs", "📄 打开文档页面").build(app)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id("quit", "🚪 退出唤星").build(app)?;

    Menu::with_items(app, &[&show_chat, &show_docs, &sep, &quit])
}
