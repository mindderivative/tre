//! 0.5.4 (#140): cursor images.
//!
//! `winit` makes a custom cursor from the event loop (`ActiveEventLoop::
//! create_custom_cursor`), which only the platform layer holds. So a cursor is
//! *registered* from anywhere on the main thread (validated at once, queued),
//! and the loop builds it the next time it runs a handler; from then on `get`
//! has it. A cursor asked for before it is built shows the default one until it
//! is. Identical images share one cursor.

use std::cell::RefCell;
use std::collections::HashMap;

use winit::event_loop::ActiveEventLoop;
use winit::window::{CustomCursor, CustomCursorSource};

#[derive(PartialEq, Eq, Hash, Clone)]
struct Key {
    rgba: Vec<u8>,
    width: u16,
    height: u16,
    hotspot: (u16, u16),
}

/// A cursor's size `(width, height)` and its hotspot.
pub type CursorInfo = ((u16, u16), (u16, u16));

#[derive(Default)]
struct Registry {
    next: u64,
    by_content: HashMap<Key, u64>,
    pending: Vec<(u64, CustomCursorSource)>,
    ready: HashMap<u64, CustomCursor>,
    /// Each id's size and hotspot, for reading a cursor back.
    info: HashMap<u64, CursorInfo>,
}

thread_local! {
    static REGISTRY: RefCell<Registry> = RefCell::new(Registry::default());
}

/// The most a cursor's side may be: what the platforms take without refusing
/// or scaling (X11 and Windows cap around 256, macOS and Wayland are looser).
pub const MAX_SIDE: u16 = 256;

/// Registers a cursor image -- straight-alpha RGBA8, `width * height * 4`
/// bytes, with its hotspot (the pixel that is the pointer's position) -- and
/// returns its id. The same image again is the same id. `Err` says what is
/// wrong with it.
pub fn register(
    rgba: Vec<u8>,
    width: u16,
    height: u16,
    hotspot: (u16, u16),
) -> Result<u64, String> {
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return Err(format!(
            "a cursor image must be 1 to {MAX_SIDE} pixels on each side, got {width} x {height}"
        ));
    }
    if rgba.len() != usize::from(width) * usize::from(height) * 4 {
        return Err(format!(
            "a {width} x {height} cursor image needs {} bytes of RGBA, got {}",
            usize::from(width) * usize::from(height) * 4,
            rgba.len()
        ));
    }
    if hotspot.0 >= width || hotspot.1 >= height {
        return Err(format!(
            "the hotspot {hotspot:?} is outside the {width} x {height} image"
        ));
    }
    REGISTRY.with(|registry| {
        let mut registry = registry.borrow_mut();
        let key = Key {
            rgba: rgba.clone(),
            width,
            height,
            hotspot,
        };
        if let Some(&id) = registry.by_content.get(&key) {
            return Ok(id);
        }
        let source = CustomCursor::from_rgba(rgba, width, height, hotspot.0, hotspot.1)
            .map_err(|err| format!("the cursor image was refused: {err}"))?;
        registry.next += 1;
        let id = registry.next;
        registry.by_content.insert(key, id);
        registry.info.insert(id, ((width, height), hotspot));
        registry.pending.push((id, source));
        Ok(id)
    })
}

/// Builds every registered cursor the loop has not yet: called from each event
/// handler, so it is cheap when nothing is waiting.
pub(crate) fn flush(event_loop: &ActiveEventLoop) {
    REGISTRY.with(|registry| {
        let mut registry = registry.borrow_mut();
        if registry.pending.is_empty() {
            return;
        }
        for (id, source) in std::mem::take(&mut registry.pending) {
            let cursor = event_loop.create_custom_cursor(source);
            registry.ready.insert(id, cursor);
        }
    });
}

/// The built cursor for `id`, once the loop has made it.
pub fn get(id: u64) -> Option<CustomCursor> {
    REGISTRY.with(|registry| registry.borrow().ready.get(&id).cloned())
}

/// The size `(width, height)` and hotspot of cursor `id`.
pub fn info(id: u64) -> Option<CursorInfo> {
    REGISTRY.with(|registry| registry.borrow().info.get(&id).copied())
}

/// Whether the loop has built cursor `id`.
pub fn is_ready(id: u64) -> bool {
    REGISTRY.with(|registry| registry.borrow().ready.contains_key(&id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: u16, height: u16, fill: u8) -> Vec<u8> {
        vec![fill; usize::from(width) * usize::from(height) * 4]
    }

    #[test]
    fn a_good_image_registers_and_the_same_image_is_the_same_cursor() {
        let a = register(image(8, 8, 200), 8, 8, (1, 1)).unwrap();
        let b = register(image(8, 8, 200), 8, 8, (1, 1)).unwrap();
        assert_eq!(a, b);
        let other_hotspot = register(image(8, 8, 200), 8, 8, (2, 1)).unwrap();
        let other_pixels = register(image(8, 8, 90), 8, 8, (1, 1)).unwrap();
        assert!(other_hotspot != a && other_pixels != a && other_hotspot != other_pixels);
        // Registered, but nothing has built them: no event loop here.
        assert!(!is_ready(a));
        assert!(get(a).is_none());
    }

    #[test]
    fn bad_images_are_refused_with_the_reason() {
        let err = |r: Result<u64, String>| r.unwrap_err();
        assert!(err(register(image(8, 8, 0), 0, 8, (0, 0))).contains("1 to"));
        assert!(err(register(image(8, 8, 0), 8, 300, (0, 0))).contains("1 to"));
        assert!(err(register(vec![0; 10], 8, 8, (0, 0))).contains("needs 256 bytes"));
        assert!(err(register(image(8, 8, 0), 8, 8, (8, 0))).contains("hotspot"));
        assert!(err(register(image(8, 8, 0), 8, 8, (0, 9))).contains("hotspot"));
    }
}
