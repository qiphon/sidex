use std::collections::HashMap;
use tauri::menu::MenuItemKind;

fn set_text_on_item<R: tauri::Runtime>(item: &MenuItemKind<R>, text: &str) {
    if let Some(mi) = item.as_menuitem() {
        let _ = mi.set_text(text);
    } else if let Some(sub) = item.as_submenu() {
        let _ = sub.set_text(text);
    }
}

/// Walk a `Menu` (or `Submenu`) tree and apply matching labels by ID.
/// `Menu::get` only searches the top level, so we recurse into submenus.
fn apply_labels<R: tauri::Runtime>(
    items: &[MenuItemKind<R>],
    labels: &mut HashMap<String, String>,
) {
    if labels.is_empty() {
        return;
    }
    for item in items {
        let id = item.id().0.clone();
        if let Some(text) = labels.remove(&id) {
            set_text_on_item(item, &text);
        }
        if let Some(sub) = item.as_submenu() {
            if let Ok(children) = sub.items() {
                apply_labels(&children, labels);
            }
        }
        if labels.is_empty() {
            return;
        }
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn update_menu_labels(
    app: tauri::AppHandle,
    labels: HashMap<String, String>,
) -> Result<(), String> {
    let menu = app.menu().ok_or("no app menu set")?;
    let items = menu.items().map_err(|e| e.to_string())?;
    let mut labels = labels;
    apply_labels(&items, &mut labels);
    Ok(())
}
