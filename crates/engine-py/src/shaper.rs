//! M96: one text shaper per thread, for measuring text outside a render --
//! a terminal's cell size, `window.measure_text`. Building a `TextRenderer`
//! registers every bundled font, so doing it per call (as the terminal
//! factories used to) repeats that work each time; this builds it once, on
//! first use, and keeps fonts registered since (`tre.register_font`) in sync.

use std::cell::RefCell;

use engine_render::TextRenderer;

thread_local! {
    static SHAPER: RefCell<Option<TextRenderer>> = const { RefCell::new(None) };
}

/// Runs `f` with this thread's shaper.
pub(crate) fn with<R>(f: impl FnOnce(&mut TextRenderer) -> R) -> R {
    SHAPER.with(|shaper| {
        let mut shaper = shaper.borrow_mut();
        let renderer = shaper.get_or_insert_with(TextRenderer::new);
        renderer.sync_registered_fonts();
        f(renderer)
    })
}
