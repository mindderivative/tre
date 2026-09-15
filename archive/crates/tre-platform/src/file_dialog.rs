//! Native file dialogs (GUI-readiness assessment recommendation #5),
//! binding `rfd::FileDialog`'s real blocking API. On Linux this is
//! backed by the XDG desktop portal (`ashpd`), confirmed against this
//! machine's real, live KDE Plasma portal implementation (both the GTK
//! and KDE portal backends are registered) in isolated feasibility
//! testing before being added here.
//!
//! Unlike [`crate::Clipboard`], there is no persistent connection to
//! hold -- each call opens a fresh native dialog and blocks until the
//! user responds, matching `rfd::FileDialog`'s own one-shot builder
//! design. These are accordingly plain functions, not a struct.

use std::path::{Path, PathBuf};

/// One real filter: a display name plus the extensions it matches (no
/// leading dot, e.g. `["txt", "md"]`) -- matching `rfd::FileDialog::
/// add_filter`'s own convention directly.
#[derive(Debug, Clone)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

/// Opens a real native "open file" dialog, blocking the calling thread
/// until the user picks a file or cancels.
///
/// Returns `None` if the user cancelled -- not an error, matching every
/// real OS file dialog's own "cancel is not a failure" convention.
#[must_use]
pub fn pick_file(
    title: Option<&str>,
    filters: &[FileFilter],
    starting_directory: Option<&Path>,
) -> Option<PathBuf> {
    build_dialog(title, filters, starting_directory).pick_file()
}

/// Opens a real native "open files" (multi-select) dialog. See
/// [`pick_file`] for the shared parameter/cancel semantics.
#[must_use]
pub fn pick_files(
    title: Option<&str>,
    filters: &[FileFilter],
    starting_directory: Option<&Path>,
) -> Option<Vec<PathBuf>> {
    build_dialog(title, filters, starting_directory).pick_files()
}

/// Opens a real native "select folder" dialog. Filters don't apply to
/// folder selection, so this takes no `filters` parameter.
#[must_use]
pub fn pick_folder(title: Option<&str>, starting_directory: Option<&Path>) -> Option<PathBuf> {
    build_dialog(title, &[], starting_directory).pick_folder()
}

/// Opens a real native "save file" dialog. `default_file_name` pre-fills
/// the dialog's file-name field (e.g. `"untitled.svg"`) -- a real,
/// common save-dialog convenience distinct from `starting_directory`.
#[must_use]
pub fn save_file(
    title: Option<&str>,
    filters: &[FileFilter],
    starting_directory: Option<&Path>,
    default_file_name: Option<&str>,
) -> Option<PathBuf> {
    let mut dialog = build_dialog(title, filters, starting_directory);
    if let Some(name) = default_file_name {
        dialog = dialog.set_file_name(name);
    }
    dialog.save_file()
}

fn build_dialog(
    title: Option<&str>,
    filters: &[FileFilter],
    starting_directory: Option<&Path>,
) -> rfd::FileDialog {
    let mut dialog = rfd::FileDialog::new();
    if let Some(title) = title {
        dialog = dialog.set_title(title);
    }
    for filter in filters {
        dialog = dialog.add_filter(&filter.name, &filter.extensions);
    }
    if let Some(dir) = starting_directory {
        dialog = dialog.set_directory(dir);
    }
    dialog
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_dialog_applies_every_real_option_without_panicking() {
        // A real, non-interactive check: constructing the builder with
        // every option set must not panic -- only `.pick_file()` etc
        // (never called in a unit test; they block on real user
        // interaction) open an actual interactive dialog.
        let _dialog = build_dialog(
            Some("Open a file"),
            &[FileFilter {
                name: "text".into(),
                extensions: vec!["txt".into(), "md".into()],
            }],
            Some(Path::new("/tmp")),
        );
    }
}
