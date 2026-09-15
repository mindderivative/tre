//! Real system clipboard access (GUI-readiness assessment
//! recommendation #4) -- wraps `arboard::Clipboard`, the same real,
//! cross-platform crate the assessment itself named as "a small,
//! self-contained crate with no architectural entanglement with the
//! rest of tre." Text only (`default-features = false` in this crate's
//! own `Cargo.toml` drops arboard's own `image-data` feature) -- a
//! real, disclosed v1 scope matching every other "smallest real slice
//! first" precedent in this project (e.g. `tre-svg`'s identical
//! `usvg = { default-features = false }`); image clipboard support is
//! real, separate future work once a caller actually needs it.

use crate::PlatformError;

fn clipboard_err(action: &str, e: arboard::Error) -> PlatformError {
    PlatformError::Other(format!("failed to {action}: {e}"))
}

/// A real handle to the system clipboard. Construct once, reuse across
/// calls -- `arboard::Clipboard::new()` opens a real connection to the
/// platform clipboard service (X11 selection owner / Wayland data
/// device / Win32 clipboard / NSPasteboard), not something cheap to
/// repeat per call.
pub struct Clipboard {
    inner: arboard::Clipboard,
}

impl Clipboard {
    /// # Errors
    /// Returns [`PlatformError::Other`] if no real clipboard service is
    /// reachable (e.g. no display server connection).
    pub fn new() -> Result<Self, PlatformError> {
        arboard::Clipboard::new()
            .map(|inner| Self { inner })
            .map_err(|e| clipboard_err("open the system clipboard", e))
    }

    /// Reads the clipboard's current real plain-text content.
    ///
    /// # Errors
    /// Returns [`PlatformError::Other`] if the clipboard is empty, holds
    /// non-text content, or the platform clipboard service itself
    /// failed.
    pub fn get_text(&mut self) -> Result<String, PlatformError> {
        self.inner
            .get_text()
            .map_err(|e| clipboard_err("read clipboard text", e))
    }

    /// Writes `text` as the clipboard's new real plain-text content,
    /// replacing whatever was there before.
    ///
    /// # Errors
    /// Returns [`PlatformError::Other`] if the platform clipboard
    /// service rejected the write.
    pub fn set_text(&mut self, text: &str) -> Result<(), PlatformError> {
        self.inner
            .set_text(text)
            .map_err(|e| clipboard_err("write clipboard text", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real round trip against this machine's own live clipboard
    /// service -- not mocked, matching this project's own "real demos/
    /// tests as the correctness oracle" precedent for platform-level
    /// code. Requires a real display server connection to be reachable,
    /// the same real, disclosed constraint every other `tre-platform`
    /// capability already has.
    #[test]
    fn set_text_then_get_text_round_trips_through_the_real_system_clipboard() {
        let mut clipboard = Clipboard::new().expect("a real clipboard service must be reachable");
        let marker = "tre-platform clipboard round-trip test";
        clipboard.set_text(marker).expect("set_text must succeed");
        let read_back = clipboard.get_text().expect("get_text must succeed");
        assert_eq!(read_back, marker);
    }
}
