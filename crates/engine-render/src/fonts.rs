//! M86: the process-global font registry -- how a caller hands `tre`
//! font data it loaded itself.
//!
//! `TextRenderer` deliberately never discovers system fonts
//! (`system_fonts: false`, see its own doc comment), so before this
//! module a node's `font_family` could name a family `tre` had no way
//! to receive -- it silently fell
//! back. The caller (a framework like Tesserae) owns reading the file;
//! `register_font` takes the bytes.
//!
//! Global rather than per-`TextRenderer`, because renderers are created
//! in several places (one per live window's GPU runtime, plus throwaway
//! ones for terminal cell metrics) and a `Window` can exist with no
//! `App` at all. Every `TextRenderer::new` registers everything here;
//! a live renderer picks up later registrations via
//! `TextRenderer::sync_registered_fonts`, driven by `generation()`.
//! The list is append-only, so a renderer only ever needs the blobs past
//! the count it has already registered.

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use parley::fontique::{Collection, CollectionOptions};
use peniko::Blob;

static REGISTERED: Mutex<Vec<Blob<u8>>> = Mutex::new(Vec::new());
/// 0.5.4 (#111): the family names of everything registered, in order. They
/// follow a node's own `font_family` in its family stack, so a glyph that
/// family lacks comes from a registered font that has it.
static FAMILIES: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Bumped (under `REGISTERED`'s lock) every time a genuinely new blob is
/// appended -- a cheap, lock-free "has anything changed" check for the
/// per-frame `sync_registered_fonts` call.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// 0.5.4 (#111): whether text may use the machine's installed fonts, for
/// families it names that aren't registered and for the glyphs the registered
/// ones lack (CJK, Hebrew, Indic, colour emoji). Off by default: text is then
/// the same on every machine, which is what tests and screenshots rely on.
static SYSTEM_FONTS: AtomicBool = AtomicBool::new(false);

/// Lets (or stops) every `TextRenderer` in this process use the system's
/// installed fonts. A live renderer picks the change up on its next
/// `sync_registered_fonts`, which then reports a change, so the next frame
/// repaints.
pub fn set_system_fonts(enabled: bool) {
    if SYSTEM_FONTS.swap(enabled, Ordering::AcqRel) != enabled {
        GENERATION.fetch_add(1, Ordering::Release);
    }
}

/// Whether system fonts are on (`set_system_fonts`).
pub fn system_fonts() -> bool {
    SYSTEM_FONTS.load(Ordering::Acquire)
}

/// `register_font` was handed bytes containing no font face `fontique`
/// could parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoFontFacesFound {
    pub byte_len: usize,
}

impl fmt::Display for NoFontFacesFound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "no font faces found in {} bytes of data -- expected a TrueType/OpenType font or collection",
            self.byte_len
        )
    }
}

impl std::error::Error for NoFontFacesFound {}

/// Registers `data` (a `.ttf`/`.otf`/`.ttc` file's raw bytes) with every
/// current and future `TextRenderer` in this process, returning the
/// family names it contains -- the exact strings a node's
/// `font_family` must use to resolve to it. Registering identical bytes
/// twice is a no-op that still returns the names.
pub fn register_font(data: Vec<u8>) -> Result<Vec<String>, NoFontFacesFound> {
    let byte_len = data.len();
    let blob = Blob::new(Arc::new(data));

    // Parse into a scratch collection first: validates the data and
    // reads its family names without touching any live renderer.
    let mut scratch = Collection::new(CollectionOptions {
        shared: false,
        system_fonts: false,
    });
    let families = scratch.register_fonts(blob.clone(), None);
    let mut names: Vec<String> = families
        .iter()
        .filter_map(|(id, _)| scratch.family_name(*id).map(str::to_owned))
        .collect();
    names.dedup();
    if names.is_empty() {
        return Err(NoFontFacesFound { byte_len });
    }

    let mut registered = REGISTERED.lock().unwrap_or_else(PoisonError::into_inner);
    if !registered.iter().any(|b| b.data() == blob.data()) {
        registered.push(blob);
        let mut families = FAMILIES.lock().unwrap_or_else(PoisonError::into_inner);
        for name in &names {
            if !families.contains(name) {
                families.push(name.clone());
            }
        }
        GENERATION.fetch_add(1, Ordering::Release);
    }
    Ok(names)
}

/// The family names registered so far (`FAMILIES`), in order.
pub(crate) fn fallback_families() -> Vec<String> {
    FAMILIES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// Current registry generation -- changes whenever `register_font`
/// appends a new blob.
pub(crate) fn generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}

/// The registered blobs from index `from` onward, plus the generation
/// they correspond to (read under the same lock, so the pair is
/// consistent) and the new total count.
pub(crate) fn registered_since(from: usize) -> (u64, usize, Vec<Blob<u8>>) {
    let registered = REGISTERED.lock().unwrap_or_else(PoisonError::into_inner);
    let generation = GENERATION.load(Ordering::Acquire);
    let new = registered
        .get(from..)
        .map(<[_]>::to_vec)
        .unwrap_or_default();
    (generation, registered.len(), new)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HACK: &[u8] = include_bytes!("../assets/fonts/HackNerdFontMono-Regular.ttf");

    #[test]
    fn registering_a_real_font_returns_its_real_family_name() {
        let names = register_font(HACK.to_vec()).expect("a real font");
        assert_eq!(names, vec!["Hack Nerd Font Mono".to_string()]);
    }

    #[test]
    fn bytes_with_no_font_faces_are_an_error_not_a_silent_no_op() {
        let err = register_font(b"definitely not a font".to_vec()).expect_err("must fail");
        assert_eq!(err, NoFontFacesFound { byte_len: 21 });
        assert!(err.to_string().contains("21 bytes"));
    }

    #[test]
    fn registering_identical_bytes_twice_stores_them_once() {
        register_font(HACK.to_vec()).unwrap();
        register_font(HACK.to_vec()).unwrap();
        let (_, _, all) = registered_since(0);
        let copies = all.iter().filter(|blob| blob.data() == HACK).count();
        assert_eq!(copies, 1, "the registry must deduplicate by content");
    }
}
