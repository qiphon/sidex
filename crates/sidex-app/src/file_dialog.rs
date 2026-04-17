//! Native file dialogs using the `rfd` crate.

use std::path::PathBuf;

/// Shows a native "Open File" dialog. Returns the selected path, or `None`
/// if the user cancelled.
pub fn open_file_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new().set_title("Open File").pick_file()
}

/// Shows a native "Save As" dialog with an optional suggested filename.
/// Returns the chosen path, or `None` if the user cancelled.
pub fn save_file_dialog(suggested: &str) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title("Save As");
    if !suggested.is_empty() {
        dialog = dialog.set_file_name(suggested);
    }
    dialog.save_file()
}

/// Shows a native "Open Folder" dialog. Returns the selected directory,
/// or `None` if the user cancelled.
pub fn open_folder_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Open Folder")
        .pick_folder()
}
