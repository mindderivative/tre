//! `tre.pick_file`/`pick_files`/`pick_folder`/`save_file` (GUI-readiness
//! assessment recommendation #5) -- real native file dialogs, binding
//! directly to `tre_platform::file_dialog`'s free functions (blocking
//! `rfd`, backed by the XDG desktop portal on Linux).
//!
//! Each call releases the GIL for its own real blocking wait (matching
//! `PyHeadlessRenderer::submit_and_read_bgra`'s own `py.detach`
//! precedent in `renderer.rs`) -- a real native dialog can block on user
//! interaction for an arbitrary, unbounded amount of real wall-clock
//! time, and other Python threads must keep running while it does.

use std::path::{Path, PathBuf};

use pyo3::prelude::*;

use tre_platform::FileFilter;

fn to_filters(filters: Vec<(String, Vec<String>)>) -> Vec<FileFilter> {
    filters
        .into_iter()
        .map(|(name, extensions)| FileFilter { name, extensions })
        .collect()
}

fn path_to_string(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

/// Opens a real native "open file" dialog, blocking until the user
/// picks a file or cancels. Returns `None` on cancel -- not an error,
/// matching every real OS file dialog's own convention.
///
/// `filters` is a list of `(display_name, extensions)` pairs, e.g.
/// `[("Images", ["png", "jpg"])]` -- extensions have no leading dot,
/// matching `rfd::FileDialog::add_filter`'s own convention directly.
#[pyfunction]
#[pyo3(signature = (title=None, filters=Vec::new(), starting_directory=None))]
fn pick_file(
    py: Python<'_>,
    title: Option<String>,
    filters: Vec<(String, Vec<String>)>,
    starting_directory: Option<String>,
) -> Option<String> {
    let filters = to_filters(filters);
    py.detach(|| {
        tre_platform::pick_file(
            title.as_deref(),
            &filters,
            starting_directory.as_deref().map(Path::new),
        )
    })
    .map(path_to_string)
}

/// Opens a real native "open files" (multi-select) dialog. See
/// [`pick_file`] for the shared parameter/cancel semantics.
#[pyfunction]
#[pyo3(signature = (title=None, filters=Vec::new(), starting_directory=None))]
fn pick_files(
    py: Python<'_>,
    title: Option<String>,
    filters: Vec<(String, Vec<String>)>,
    starting_directory: Option<String>,
) -> Option<Vec<String>> {
    let filters = to_filters(filters);
    py.detach(|| {
        tre_platform::pick_files(
            title.as_deref(),
            &filters,
            starting_directory.as_deref().map(Path::new),
        )
    })
    .map(|paths| paths.into_iter().map(path_to_string).collect())
}

/// Opens a real native "select folder" dialog. Filters don't apply to
/// folder selection, so this takes no `filters` parameter.
#[pyfunction]
#[pyo3(signature = (title=None, starting_directory=None))]
fn pick_folder(
    py: Python<'_>,
    title: Option<String>,
    starting_directory: Option<String>,
) -> Option<String> {
    py.detach(|| {
        tre_platform::pick_folder(
            title.as_deref(),
            starting_directory.as_deref().map(Path::new),
        )
    })
    .map(path_to_string)
}

/// Opens a real native "save file" dialog. `default_file_name` pre-fills
/// the dialog's file-name field (e.g. `"untitled.svg"`) -- a real,
/// common save-dialog convenience distinct from `starting_directory`.
#[pyfunction]
#[pyo3(signature = (title=None, filters=Vec::new(), starting_directory=None, default_file_name=None))]
fn save_file(
    py: Python<'_>,
    title: Option<String>,
    filters: Vec<(String, Vec<String>)>,
    starting_directory: Option<String>,
    default_file_name: Option<String>,
) -> Option<String> {
    let filters = to_filters(filters);
    py.detach(|| {
        tre_platform::save_file(
            title.as_deref(),
            &filters,
            starting_directory.as_deref().map(Path::new),
            default_file_name.as_deref(),
        )
    })
    .map(path_to_string)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(pick_file, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(pick_files, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(pick_folder, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(save_file, m)?)?;
    Ok(())
}
